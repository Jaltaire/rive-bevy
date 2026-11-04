use std::collections::HashMap;
#[cfg(not(metal_renderer_native))]
use std::sync::Arc;

#[cfg(metal_renderer_native)]
use bevy::render::renderer::RenderDevice;
#[cfg(not(metal_renderer_native))]
use bevy::ecs::batching::BatchingStrategy;
#[cfg(not(metal_renderer_native))]
use bevy::render::renderer::RenderAdapter;
use bevy::{
    core_pipeline::{Core2d, Core2dSystems, Core3d, Core3dSystems},
    prelude::*,
    render::{
        extract_component::ExtractComponentPlugin, render_resource::DownlevelFlags, Render,
        RenderApp, RenderSystems,
    },
};
#[cfg(not(metal_renderer_native))]
use rive_rs::Instantiate;

#[cfg(metal_renderer_native)]
use crate::metal::{
    instantiate_linear_animations, instantiate_state_machines, pass_state_machine_input_events,
    render_rive_scenes,
};
#[cfg(not(metal_renderer_native))]
use crate::{
    components::{
        MissingArtboard, MissingLinearAnimation, MissingStateMachine, VelloFragment,
    },
    events::InputValue,
    node,
};
use crate::{
    assets::{self, Riv, RivLoader},
    components::{
        LinearAnimation, RiveLinearAnimation, RiveStateMachine, SceneImage, StateMachine,
        Viewport,
    },
    events::{GenericEvent, Input},
};
#[cfg(feature = "pointer-events")]
use crate::pointer_events;

#[cfg(not(metal_renderer_native))]
pub(crate) type DynScene = dyn rive_rs::Scene;

#[cfg(metal_renderer_native)]
pub(crate) type DynScene = dyn crate::metal::NativeScene;

macro_rules! get_scene_or {
    ( $keyword:tt, $linear_animation:expr, $state_machine:expr ) => {{
        let linear_animation = $linear_animation
            .map(|la| la.map_unchanged(|la| (&mut **la) as &mut crate::plugin::DynScene));
        let state_machine = $state_machine
            .map(|sm| sm.map_unchanged(|sm| (&mut **sm) as &mut crate::plugin::DynScene));

        match (linear_animation, state_machine) {
            (Some(linear_animation), None) => linear_animation,
            (None, Some(state_machine)) => state_machine,
            _ => $keyword,
        }
    }};
}

#[cfg(feature = "pointer-events")]
pub(crate) use get_scene_or;

#[cfg(not(metal_renderer_native))]
macro_rules! get_or_continue_with_error {
    ( $val:expr, $( $tail:tt )* ) => {
        match $val {
            Some(val) => val,
            None => {
                error!($($tail)*);
                continue;
            }
        }
    };
}

fn insert_deafult_viewports(
    mut commands: Commands,
    query: Query<
        (Entity, &SceneImage),
        (
            Or<(Added<LinearAnimation>, Added<StateMachine>)>,
            Without<Viewport>,
        ),
    >,
    image_assets: Res<Assets<Image>>,
) {
    for (entity, image_handle) in &query {
        let mut viewport = Viewport::default();

        if let Some(image) = image_assets.get(&image_handle.0) {
            let size = image.size();
            viewport.resize(size.x, size.y);
        }

        commands.entity(entity).insert(viewport);
    }
}

fn resize_viewports(
    mut query: Query<(&mut Viewport, &SceneImage)>,
    image_assets: Res<Assets<Image>>,
) {
    for (mut viewport, image_handle) in &mut query {
        if let Some(image) = image_assets.get(&image_handle.0) {
            let size = image.size();
            viewport.resize(size.x, size.y);
        }
    }
}

#[derive(Debug, Default, Deref, DerefMut, Resource)]
pub(crate) struct RivEntities(HashMap<AssetId<assets::Riv>, Entity>);

#[cfg(not(metal_renderer_native))]
fn instantiate_linear_animations(
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
    riv_assets: Res<Assets<assets::Riv>>,
    mut riv_entities: ResMut<RivEntities>,
) {
    for (entity, linear_animation, missing_artboard, missing_linear_animation) in &query {
        if let Some(riv) = riv_assets.get(&linear_animation.riv) {
            let handle = linear_animation.riv.clone();
            let artboard =
                match rive_rs::Artboard::instantiate(riv, linear_animation.artboard_handle.clone())
                {
                    Some(artboard) => artboard,
                    None => {
                        if missing_artboard.is_none() {
                            commands.entity(entity).insert(MissingArtboard);

                            error!(
                                "artboard {:?} cannot be found in {:?}",
                                linear_animation.artboard_handle, riv,
                            );
                        }

                        continue;
                    }
                };

            commands.entity(entity).remove::<MissingArtboard>();

            let linear_animation = match rive_rs::LinearAnimation::instantiate(
                &artboard,
                linear_animation.handle.clone(),
            ) {
                Some(linear_animation) => linear_animation,
                None => {
                    if missing_linear_animation.is_none() {
                        commands.entity(entity).insert(MissingLinearAnimation);

                        error!(
                            "linear animation {:?} cannot be found in {:?}",
                            linear_animation.handle, riv,
                        );
                    }

                    continue;
                }
            };

            commands.entity(entity).remove::<MissingLinearAnimation>();

            commands
                .entity(entity)
                .insert(RiveLinearAnimation(linear_animation));

            riv_entities.insert(handle.id(), entity);
        }
    }
}

#[cfg(not(metal_renderer_native))]
fn instantiate_state_machines(
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
    riv_assets: Res<Assets<assets::Riv>>,
    mut riv_entities: ResMut<RivEntities>,
) {
    for (entity, state_machine, missing_artboard, missing_state_machine) in &query {
        if let Some(riv) = riv_assets.get(&state_machine.riv) {
            let handle = state_machine.riv.clone();
            let artboard =
                match rive_rs::Artboard::instantiate(riv, state_machine.artboard_handle.clone()) {
                    Some(artboard) => artboard,
                    None => {
                        if missing_artboard.is_none() {
                            commands.entity(entity).insert(MissingArtboard);

                            error!(
                                "artboard {:?} cannot be found in {:?}",
                                state_machine.artboard_handle, riv,
                            );
                        }

                        continue;
                    }
                };

            commands.entity(entity).remove::<MissingArtboard>();

            let state_machine =
                match rive_rs::StateMachine::instantiate(&artboard, state_machine.handle.clone()) {
                    Some(state_machine) => state_machine,
                    None => {
                        if missing_state_machine.is_none() {
                            commands.entity(entity).insert(MissingStateMachine);

                            error!(
                                "linear animation {:?} cannot be found in {:?}",
                                state_machine.handle, riv,
                            );
                        }

                        continue;
                    }
                };

            commands.entity(entity).remove::<MissingStateMachine>();

            commands
                .entity(entity)
                .insert(RiveStateMachine(state_machine));

            riv_entities.insert(handle.id(), entity);
        }
    }
}

fn reinstantiate_linear_animations(
    mut commands: Commands,
    mut asset_events: MessageReader<AssetEvent<assets::Riv>>,
    mut riv_entities: ResMut<RivEntities>,
) {
    for event in asset_events.read() {
        match event {
            AssetEvent::Modified { id } => {
                commands
                    .entity(*riv_entities.get(id).unwrap())
                    .remove::<RiveLinearAnimation>()
                    .remove::<RiveStateMachine>();
            }
            AssetEvent::Removed { id } => {
                riv_entities.remove(id);
            }
            _ => (),
        }
    }
}

#[cfg(not(metal_renderer_native))]
fn pass_state_machine_input_events(
    mut query: Query<&mut RiveStateMachine>,
    mut input_events: MessageReader<Input>,
) {
    for input in input_events.read() {
        if let Ok(state_machine) = query.get_mut(input.state_machine) {
            match input.value {
                InputValue::Bool(val) => get_or_continue_with_error!(
                    state_machine.get_bool(&input.name),
                    "input with name {:?} cannot be found in {:?}",
                    input.name,
                    input.state_machine,
                )
                .set(val),
                InputValue::Number(val) => get_or_continue_with_error!(
                    state_machine.get_number(&input.name),
                    "input with name {:?} cannot be found in {:?}",
                    input.name,
                    input.state_machine,
                )
                .set(val),
                InputValue::Trigger => get_or_continue_with_error!(
                    state_machine.get_trigger(&input.name),
                    "input with name {:?} cannot be found in {:?}",
                    input.name,
                    input.state_machine,
                )
                .fire(),
            }
        }
    }
}

#[cfg(not(metal_renderer_native))]
fn render_rive_scenes(
    time: Res<Time>,
    par_commands: ParallelCommands,
    mut query: Query<(
        Entity,
        Option<&mut RiveLinearAnimation>,
        Option<&mut RiveStateMachine>,
        &mut Viewport,
        Option<&crate::components::ArtboardContentRect>,
    )>,
) {
    const MAX_SCENES_PER_CORE: usize = 8;

    let elapsed = time.delta();

    query
        .par_iter_mut()
        .batching_strategy(BatchingStrategy::new().max_batch_size(MAX_SCENES_PER_CORE))
        .for_each(|(entity, linear_animation, state_machine, mut viewport, content_rect)| {
            match content_rect {
                Some(rect) => {
                    viewport.set_content_rect(rect.x, rect.y, rect.width, rect.height)
                }
                None => viewport.clear_content_rect(),
            }
            let mut renderer = rive_rs::Renderer::default();
            let mut scene = get_scene_or!(return, linear_animation, state_machine);

            par_commands.command_scope(|mut commands| {
                if scene.advance_and_maybe_draw(&mut renderer, elapsed, &mut viewport) {
                    commands
                        .entity(entity)
                        .insert(VelloFragment(Arc::new(renderer.into_scene())));
                } else {
                    commands.entity(entity).remove::<VelloFragment>();
                }
            });
        });
}

fn send_generic_events(
    query: Query<(Entity, &RiveStateMachine)>,
    mut generic_events: MessageWriter<GenericEvent>,
) {
    for (entity, state_machine) in &query {
        for event in state_machine.events() {
            generic_events.write(GenericEvent {
                state_machine: entity,
                name: event.name,
                delay: event.delay,
                properties: event.properties,
            });
        }
    }
}

#[cfg(not(metal_renderer_native))]
fn reset_renderer(context: Res<node::VelloContext>) {
    context.reset_renderer();
}

#[cfg(not(metal_renderer_native))]
pub const REQUIRED_DOWNLEVEL_FLAGS: DownlevelFlags = DownlevelFlags::COMPUTE_SHADERS
    .union(DownlevelFlags::INDIRECT_EXECUTION)
    .union(DownlevelFlags::SHADER_F16_IN_F32);

#[cfg(metal_renderer_native)]
pub const REQUIRED_DOWNLEVEL_FLAGS: DownlevelFlags = DownlevelFlags::empty();

pub fn missing_downlevel_flags(flags: DownlevelFlags) -> DownlevelFlags {
    REQUIRED_DOWNLEVEL_FLAGS.difference(flags)
}

pub fn renders_on(flags: DownlevelFlags) -> bool {
    missing_downlevel_flags(flags).is_empty()
}

#[cfg(not(metal_renderer_native))]
fn add_vello_rendering(render_app: &mut SubApp, flags: DownlevelFlags) {
    let missing = missing_downlevel_flags(flags);
    if !missing.is_empty() {
        warn!("The GPU lacks {missing:?}, which Vello needs, so Rive scenes will not be drawn.");
        return;
    }

    render_app
        .init_resource::<node::VelloContext>()
        .add_systems(Render, reset_renderer.in_set(RenderSystems::Cleanup))
        .add_systems(
            Core2d,
            node::render_vello_scene_textures.before(Core2dSystems::MainPass),
        )
        .add_systems(
            Core3d,
            node::render_vello_scene_textures.before(Core3dSystems::MainPass),
        );
}

pub struct RivePlugin;

impl Plugin for RivePlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<Riv>()
            .init_asset_loader::<RivLoader>()
            .init_resource::<RivEntities>()
            .add_message::<Input>()
            .add_message::<GenericEvent>()
            .add_systems(
                PreUpdate,
                (
                    (insert_deafult_viewports, resize_viewports).chain(),
                    reinstantiate_linear_animations,
                    instantiate_linear_animations,
                    instantiate_state_machines,
                ),
            )
            .add_systems(
                Update,
                (
                    pass_state_machine_input_events,
                    send_generic_events,
                    render_rive_scenes,
                )
                    .chain(),
            );

        #[cfg(feature = "pointer-events")]
        app.add_systems(
            Update,
            pointer_events::pass.before(pass_state_machine_input_events),
        );

        #[cfg(not(metal_renderer_native))]
        app.add_plugins(ExtractComponentPlugin::<VelloFragment>::default());

        #[cfg(metal_renderer_native)]
        app.init_resource::<crate::metal::MetalFiles>()
            .add_systems(
                PreUpdate,
                crate::metal::evict_metal_files
                    .before(instantiate_linear_animations)
                    .before(instantiate_state_machines),
            )
            .add_plugins(ExtractComponentPlugin::<crate::metal::MetalDrawRequest>::default());
    }

    #[cfg(not(metal_renderer_native))]
    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        let flags = render_app
            .world()
            .resource::<RenderAdapter>()
            .get_downlevel_capabilities()
            .flags;
        add_vello_rendering(render_app, flags);
    }

    #[cfg(metal_renderer_native)]
    fn finish(&self, app: &mut App) {
        let Some(render_device) = app.world().get_resource::<RenderDevice>() else {
            return;
        };
        let context = crate::metal::create_render_context(render_device);
        app.insert_resource(context.clone());

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .insert_resource(context)
            .init_resource::<crate::metal_node::MetalTargets>()
            .add_systems(
                Render,
                crate::metal_node::reset_targets.in_set(RenderSystems::Cleanup),
            )
            .add_systems(
                Core2d,
                crate::metal_node::render_metal_scene_textures.before(Core2dSystems::MainPass),
            )
            .add_systems(
                Core3d,
                crate::metal_node::render_metal_scene_textures.before(Core3dSystems::MainPass),
            );
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(metal_renderer_native))]
    use bevy::ecs::schedule::ScheduleLabel;

    use super::*;

    #[test]
    fn an_adapter_with_every_flag_renders() {
        assert!(renders_on(DownlevelFlags::all()));
        assert!(missing_downlevel_flags(DownlevelFlags::all()).is_empty());
    }

    #[test]
    fn an_adapter_with_exactly_the_required_flags_renders() {
        assert!(renders_on(REQUIRED_DOWNLEVEL_FLAGS));
    }

    #[test]
    fn flags_the_renderer_does_not_use_do_not_matter() {
        let flags = DownlevelFlags::all()
            .difference(DownlevelFlags::FRAGMENT_WRITABLE_STORAGE | DownlevelFlags::VERTEX_STORAGE);
        assert!(renders_on(flags));
    }

    #[cfg(not(metal_renderer_native))]
    #[test]
    fn vello_needs_compute_indirect_dispatch_and_half_float_packing() {
        assert_eq!(
            REQUIRED_DOWNLEVEL_FLAGS,
            DownlevelFlags::COMPUTE_SHADERS
                | DownlevelFlags::INDIRECT_EXECUTION
                | DownlevelFlags::SHADER_F16_IN_F32
        );
    }

    #[cfg(not(metal_renderer_native))]
    #[test]
    fn an_adapter_with_no_flags_lacks_every_required_one() {
        assert!(!renders_on(DownlevelFlags::empty()));
        assert_eq!(
            missing_downlevel_flags(DownlevelFlags::empty()),
            REQUIRED_DOWNLEVEL_FLAGS
        );
    }

    #[cfg(not(metal_renderer_native))]
    #[test]
    fn an_adapter_lacking_any_one_required_flag_does_not_render() {
        for flag in REQUIRED_DOWNLEVEL_FLAGS.iter() {
            let flags = DownlevelFlags::all().difference(flag);
            assert!(!renders_on(flags), "{flag:?} is required.");
            assert_eq!(missing_downlevel_flags(flags), flag);
        }
    }

    #[cfg(not(metal_renderer_native))]
    #[test]
    fn an_adapter_lacking_a_required_flag_gets_no_vello_renderer() {
        for flag in REQUIRED_DOWNLEVEL_FLAGS.iter() {
            let mut render_app = SubApp::new();
            add_vello_rendering(&mut render_app, DownlevelFlags::all().difference(flag));
            assert!(
                !render_app.world().contains_resource::<node::VelloContext>(),
                "Without {flag:?}, no Vello renderer is made."
            );
            for label in [Render.intern(), Core2d.intern(), Core3d.intern()] {
                assert!(
                    render_app
                        .world()
                        .resource::<Schedules>()
                        .get(label)
                        .is_none(),
                    "Without {flag:?}, nothing is scheduled in {label:?}."
                );
            }
        }
    }

    #[cfg(metal_renderer_native)]
    #[test]
    fn the_native_metal_renderer_needs_no_downlevel_flag() {
        assert!(REQUIRED_DOWNLEVEL_FLAGS.is_empty());
        assert!(renders_on(DownlevelFlags::empty()));
    }
}
