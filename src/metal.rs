use std::{
    collections::HashMap,
    ffi::c_void,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use bevy::{
    ecs::batching::BatchingStrategy,
    prelude::*,
    render::{
        extract_component::ExtractComponent, renderer::RenderDevice,
        sync_component::SyncComponent,
    },
};
use rive_rs::metal_renderer::{
    Alignment, Fit, MetalArtboard, MetalContext, MetalFile, MetalLinearAnimation,
    MetalStateMachine,
};

use crate::{
    assets::Riv,
    components::{
        LinearAnimation, MissingArtboard, MissingLinearAnimation, MissingStateMachine,
        RiveLinearAnimation, RiveStateMachine, SceneImage, StateMachine, Viewport,
    },
    events::{Input, InputValue},
    plugin::{get_scene_or, RivEntities},
};

fn transform_point(x: f32, y: f32, transform: &[f32; 6]) -> [f32; 2] {
    [
        transform[0] * x + transform[2] * y + transform[4],
        transform[1] * x + transform[3] * y + transform[5],
    ]
}

pub(crate) trait NativeScene: Send + Sync {
    fn advance_and_apply(&mut self, elapsed: Duration) -> bool;
    fn pointer_down(&mut self, x: f32, y: f32, viewport: &rive_rs::Viewport);
    fn pointer_move(&mut self, x: f32, y: f32, viewport: &rive_rs::Viewport);
    fn pointer_up(&mut self, x: f32, y: f32, viewport: &rive_rs::Viewport);
    fn draw_handle(&self) -> MetalDrawHandle;
}

#[derive(Debug)]
pub struct NativeStateMachine {
    state_machine: MetalStateMachine,
    artboard: MetalArtboard,
    artboard_lock: Arc<Mutex<()>>,
    has_drawn: Arc<AtomicBool>,
}

impl NativeStateMachine {
    fn new(artboard: MetalArtboard, mut state_machine: MetalStateMachine) -> Self {
        // The C++ runtime clears every trigger at the end of each advance, and
        // a state machine's first advance can spend all of its transition
        // evaluations settling out of the entry state. Settling the machine
        // here guarantees that triggers fired on the same frame the machine is
        // instantiated are evaluated against the settled state instead of
        // being consumed by the entry transitions.
        state_machine.advance_and_apply(Duration::ZERO);

        Self {
            state_machine,
            artboard,
            artboard_lock: Arc::new(Mutex::new(())),
            has_drawn: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn set_bool(&mut self, name: &str, value: bool) -> bool {
        self.state_machine.set_bool(name, value)
    }

    pub fn set_number(&mut self, name: &str, value: f32) -> bool {
        self.state_machine.set_number(name, value)
    }

    pub fn fire_trigger(&mut self, name: &str) -> bool {
        self.state_machine.fire_trigger(name)
    }

    pub fn get_bool(&self, name: &str) -> Option<bool> {
        self.state_machine.get_bool(name)
    }

    pub fn get_number(&self, name: &str) -> Option<f32> {
        self.state_machine.get_number(name)
    }

    pub fn events(&self) -> core::iter::Empty<rive_rs::state_machine::Event> {
        core::iter::empty()
    }

    fn artboard_pointer_position(&self, x: f32, y: f32, viewport: &rive_rs::Viewport) -> [f32; 2] {
        let (_, inverse_view_transform) = self.artboard.alignment_transforms(
            Fit::Contain,
            Alignment::CENTER,
            1.0,
            viewport.width(),
            viewport.height(),
        );

        transform_point(x, y, &inverse_view_transform)
    }
}

impl NativeScene for NativeStateMachine {
    fn advance_and_apply(&mut self, elapsed: Duration) -> bool {
        let _artboard_guard = self.artboard_lock.lock().unwrap();
        self.state_machine.advance_and_apply(elapsed)
    }

    fn pointer_down(&mut self, x: f32, y: f32, viewport: &rive_rs::Viewport) {
        let [x, y] = self.artboard_pointer_position(x, y, viewport);
        self.state_machine.pointer_down(x, y);
    }

    fn pointer_move(&mut self, x: f32, y: f32, viewport: &rive_rs::Viewport) {
        let [x, y] = self.artboard_pointer_position(x, y, viewport);
        self.state_machine.pointer_move(x, y);
    }

    fn pointer_up(&mut self, x: f32, y: f32, viewport: &rive_rs::Viewport) {
        let [x, y] = self.artboard_pointer_position(x, y, viewport);
        self.state_machine.pointer_up(x, y);
    }

    fn draw_handle(&self) -> MetalDrawHandle {
        MetalDrawHandle {
            artboard: self.artboard.clone(),
            artboard_lock: Arc::clone(&self.artboard_lock),
            has_drawn: Arc::clone(&self.has_drawn),
        }
    }
}

#[derive(Debug)]
pub struct NativeLinearAnimation {
    linear_animation: MetalLinearAnimation,
    artboard: MetalArtboard,
    artboard_lock: Arc<Mutex<()>>,
    has_drawn: Arc<AtomicBool>,
}

impl NativeLinearAnimation {
    fn new(artboard: MetalArtboard, linear_animation: MetalLinearAnimation) -> Self {
        Self {
            linear_animation,
            artboard,
            artboard_lock: Arc::new(Mutex::new(())),
            has_drawn: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl NativeScene for NativeLinearAnimation {
    fn advance_and_apply(&mut self, elapsed: Duration) -> bool {
        let _artboard_guard = self.artboard_lock.lock().unwrap();
        self.linear_animation.advance_and_apply(elapsed)
    }

    // The C++ runtime's base scene ignores pointer events for linear
    // animations, so these are intentional no-ops to match the vello path.
    fn pointer_down(&mut self, _x: f32, _y: f32, _viewport: &rive_rs::Viewport) {}

    fn pointer_move(&mut self, _x: f32, _y: f32, _viewport: &rive_rs::Viewport) {}

    fn pointer_up(&mut self, _x: f32, _y: f32, _viewport: &rive_rs::Viewport) {}

    fn draw_handle(&self) -> MetalDrawHandle {
        MetalDrawHandle {
            artboard: self.artboard.clone(),
            artboard_lock: Arc::clone(&self.artboard_lock),
            has_drawn: Arc::clone(&self.has_drawn),
        }
    }
}

#[derive(Clone)]
pub(crate) struct MetalDrawHandle {
    pub(crate) artboard: MetalArtboard,
    pub(crate) artboard_lock: Arc<Mutex<()>>,
    pub(crate) has_drawn: Arc<AtomicBool>,
}

#[derive(Clone, Component)]
pub(crate) struct MetalDrawRequest(pub(crate) MetalDrawHandle);

#[derive(Component)]
pub(crate) struct MetalSceneDraw {
    pub(crate) handle: MetalDrawHandle,
    pub(crate) image_handle: Handle<Image>,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl SyncComponent for MetalDrawRequest {
    type Target = MetalSceneDraw;
}

impl ExtractComponent for MetalDrawRequest {
    type QueryData = (
        &'static MetalDrawRequest,
        &'static SceneImage,
        &'static Viewport,
    );

    type QueryFilter = ();

    type Out = MetalSceneDraw;

    fn extract_component(
        (request, image, viewport): bevy::ecs::query::QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out> {
        Some(MetalSceneDraw {
            handle: request.0.clone(),
            image_handle: image.0.clone(),
            width: viewport.width(),
            height: viewport.height(),
        })
    }
}

#[derive(Clone, Resource)]
pub(crate) struct MetalRenderContext(pub(crate) Arc<Mutex<MetalContext>>);

pub(crate) enum ImportedRiv {
    Imported(MetalFile),
    Failed,
}

#[derive(Default, Resource)]
pub(crate) struct MetalFiles(HashMap<AssetId<Riv>, ImportedRiv>);

pub(crate) fn create_render_context(render_device: &RenderDevice) -> MetalRenderContext {
    let raw_device = unsafe {
        render_device
            .wgpu_device()
            .as_hal::<wgpu::hal::api::Metal>()
            .map(|hal_device| {
                let device_object: *const _ = &**hal_device.raw_device();
                device_object as *mut c_void
            })
    }
    .expect(
        "The metal-renderer feature requires the Metal wgpu backend, but the render device is not backed by Metal.",
    );

    let context = unsafe { MetalContext::new(raw_device) }
        .expect("The Rive Metal render context failed to initialize.");

    MetalRenderContext(Arc::new(Mutex::new(context)))
}

fn imported_file<'f>(
    metal_files: &'f mut MetalFiles,
    context: &MetalRenderContext,
    riv_handle: &Handle<Riv>,
    riv: &Riv,
) -> Option<&'f MetalFile> {
    let imported = metal_files.0.entry(riv_handle.id()).or_insert_with(|| {
        match context.0.lock().unwrap().import_file(&riv.0) {
            Ok(file) => ImportedRiv::Imported(file),
            Err(error) => {
                error!("The Rive file {:?} failed to import: {error}.", riv_handle);
                ImportedRiv::Failed
            }
        }
    });

    match imported {
        ImportedRiv::Imported(file) => Some(file),
        ImportedRiv::Failed => None,
    }
}

pub(crate) fn evict_metal_files(
    mut asset_events: MessageReader<AssetEvent<Riv>>,
    mut metal_files: ResMut<MetalFiles>,
) {
    for event in asset_events.read() {
        match event {
            AssetEvent::Modified { id } | AssetEvent::Removed { id } => {
                metal_files.0.remove(id);
            }
            _ => (),
        }
    }
}

pub(crate) fn instantiate_linear_animations(
    mut commands: Commands,
    query: Query<
        (
            Entity,
            &LinearAnimation,
            Option<&MissingArtboard>,
            Option<&MissingLinearAnimation>,
        ),
        Without<RiveLinearAnimation>,
    >,
    riv_assets: Res<Assets<Riv>>,
    mut metal_files: ResMut<MetalFiles>,
    context: Option<Res<MetalRenderContext>>,
    mut riv_entities: ResMut<RivEntities>,
) {
    let Some(context) = context else {
        return;
    };
    for (entity, linear_animation, missing_artboard, missing_linear_animation) in &query {
        if let Some(riv) = riv_assets.get(&linear_animation.riv) {
            let handle = linear_animation.riv.clone();
            let Some(file) = imported_file(&mut metal_files, &context, &handle, riv) else {
                continue;
            };

            let artboard =
                match file.instantiate_artboard(linear_animation.artboard_handle.clone()) {
                    Some(artboard) => artboard,
                    None => {
                        if missing_artboard.is_none() {
                            commands.entity(entity).insert(MissingArtboard);

                            error!(
                                "Artboard {:?} cannot be found in {:?}.",
                                linear_animation.artboard_handle, handle,
                            );
                        }

                        continue;
                    }
                };

            commands.entity(entity).remove::<MissingArtboard>();

            let instance =
                match artboard.instantiate_linear_animation(linear_animation.handle.clone()) {
                    Some(instance) => instance,
                    None => {
                        if missing_linear_animation.is_none() {
                            commands.entity(entity).insert(MissingLinearAnimation);

                            error!(
                                "Linear animation {:?} cannot be found in {:?}.",
                                linear_animation.handle, handle,
                            );
                        }

                        continue;
                    }
                };

            commands.entity(entity).remove::<MissingLinearAnimation>();

            commands
                .entity(entity)
                .insert(RiveLinearAnimation(NativeLinearAnimation::new(
                    artboard, instance,
                )));

            riv_entities.insert(handle.id(), entity);
        }
    }
}

pub(crate) fn instantiate_state_machines(
    mut commands: Commands,
    query: Query<
        (
            Entity,
            &StateMachine,
            Option<&MissingArtboard>,
            Option<&MissingStateMachine>,
        ),
        Without<RiveStateMachine>,
    >,
    riv_assets: Res<Assets<Riv>>,
    mut metal_files: ResMut<MetalFiles>,
    context: Option<Res<MetalRenderContext>>,
    mut riv_entities: ResMut<RivEntities>,
) {
    let Some(context) = context else {
        return;
    };
    for (entity, state_machine, missing_artboard, missing_state_machine) in &query {
        if let Some(riv) = riv_assets.get(&state_machine.riv) {
            let handle = state_machine.riv.clone();
            let Some(file) = imported_file(&mut metal_files, &context, &handle, riv) else {
                continue;
            };

            let artboard = match file.instantiate_artboard(state_machine.artboard_handle.clone())
            {
                Some(artboard) => artboard,
                None => {
                    if missing_artboard.is_none() {
                        commands.entity(entity).insert(MissingArtboard);

                        error!(
                            "Artboard {:?} cannot be found in {:?}.",
                            state_machine.artboard_handle, handle,
                        );
                    }

                    continue;
                }
            };

            commands.entity(entity).remove::<MissingArtboard>();

            let instance = match artboard.instantiate_state_machine(state_machine.handle.clone())
            {
                Some(instance) => instance,
                None => {
                    if missing_state_machine.is_none() {
                        commands.entity(entity).insert(MissingStateMachine);

                        error!(
                            "State machine {:?} cannot be found in {:?}.",
                            state_machine.handle, handle,
                        );
                    }

                    continue;
                }
            };

            commands.entity(entity).remove::<MissingStateMachine>();

            commands
                .entity(entity)
                .insert(RiveStateMachine(NativeStateMachine::new(
                    artboard, instance,
                )));

            riv_entities.insert(handle.id(), entity);
        }
    }
}

pub(crate) fn pass_state_machine_input_events(
    mut query: Query<&mut RiveStateMachine>,
    mut input_events: MessageReader<Input>,
) {
    for input in input_events.read() {
        if let Ok(mut state_machine) = query.get_mut(input.state_machine) {
            let input_was_found = match input.value {
                InputValue::Bool(val) => state_machine.set_bool(&input.name, val),
                InputValue::Number(val) => state_machine.set_number(&input.name, val),
                InputValue::Trigger => state_machine.fire_trigger(&input.name),
            };

            if !input_was_found {
                error!(
                    "Input with name {:?} cannot be found in {:?}.",
                    input.name, input.state_machine,
                );
            }
        }
    }
}

pub(crate) fn render_rive_scenes(
    time: Res<Time>,
    par_commands: ParallelCommands,
    mut query: Query<(
        Entity,
        Option<&mut RiveLinearAnimation>,
        Option<&mut RiveStateMachine>,
    )>,
) {
    const MAX_SCENES_PER_CORE: usize = 8;

    let elapsed = time.delta();

    query
        .par_iter_mut()
        .batching_strategy(BatchingStrategy::new().max_batch_size(MAX_SCENES_PER_CORE))
        .for_each(|(entity, linear_animation, state_machine)| {
            let mut scene = get_scene_or!(return, linear_animation, state_machine);

            par_commands.command_scope(|mut commands| {
                let advanced = scene.advance_and_apply(elapsed);
                let handle = scene.draw_handle();

                if advanced || !handle.has_drawn.load(Ordering::Relaxed) {
                    commands.entity(entity).insert(MetalDrawRequest(handle));
                } else {
                    commands.entity(entity).remove::<MetalDrawRequest>();
                }
            });
        });
}
