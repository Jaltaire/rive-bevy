#![cfg(all(metal_renderer_native, target_os = "macos"))]

use std::sync::{Arc, Mutex};

use bevy::{
    app::PluginsState,
    asset::RenderAssetUsages,
    camera::RenderTarget,
    prelude::*,
    render::{
        gpu_readback::{Readback, ReadbackComplete},
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
    },
    window::ExitCondition,
    winit::WinitPlugin,
};
use rive_bevy::{RivePlugin, SceneTarget, StateMachine};

const WIDTH: u32 = 256;
const HEIGHT: u32 = 256;
const MAX_UPDATES: usize = 300;

#[test]
fn renders_a_state_machine_into_the_target_image() {
    let captured: Arc<Mutex<Option<Vec<u8>>>> = Arc::default();
    let captured_in_observer = Arc::clone(&captured);

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .disable::<WinitPlugin>(),
    )
    .add_plugins(RivePlugin);

    while app.plugins_state() == PluginsState::Adding {
        bevy::tasks::tick_global_task_pools_on_main_thread();
    }
    app.finish();
    app.cleanup();

    let riv = app
        .world()
        .resource::<AssetServer>()
        .load("rating-animation.riv");

    let mut images = app.world_mut().resource_mut::<Assets<Image>>();

    let mut scene_image = Image::new_fill(
        Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    scene_image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
        | TextureUsages::COPY_DST
        | TextureUsages::COPY_SRC
        | TextureUsages::RENDER_ATTACHMENT;
    let scene_image = images.add(scene_image);

    let mut camera_image = Image::new_fill(
        Extent3d {
            width: 16,
            height: 16,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    camera_image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT;
    let camera_image = images.add(camera_image);

    app.world_mut().spawn((
        StateMachine {
            riv,
            ..default()
        },
        SceneTarget {
            image: scene_image.clone().into(),
            ..default()
        },
    ));

    app.world_mut().spawn((
        Camera2d,
        RenderTarget::Image(camera_image.into()),
    ));

    app.world_mut()
        .spawn(Readback::texture(scene_image.clone()))
        .observe(move |event: On<ReadbackComplete>| {
            let mut captured = captured_in_observer.lock().unwrap();
            if captured.is_none() && event.data.iter().any(|&byte| byte != 0) {
                *captured = Some(event.data.clone());
            }
        });

    for _ in 0..MAX_UPDATES {
        app.update();

        if captured.lock().unwrap().is_some() {
            break;
        }
    }

    let pixels = captured
        .lock()
        .unwrap()
        .take()
        .expect("The Rive scene never produced non-zero pixels in the target image.");

    assert_eq!(pixels.len(), (WIDTH * HEIGHT * 4) as usize);

    let mut distinct = std::collections::BTreeSet::new();
    for pixel in pixels.chunks_exact(4) {
        distinct.insert(<[u8; 4]>::try_from(pixel).unwrap());
    }
    assert!(
        distinct.len() > 8,
        "The target image holds too few distinct pixel values for a drawn artboard: {}.",
        distinct.len()
    );
}
