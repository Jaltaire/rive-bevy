#![cfg(not(metal_renderer_native))]

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use bevy::{
    app::PluginsState,
    asset::RenderAssetUsages,
    camera::RenderTarget,
    prelude::*,
    render::{
        gpu_readback::{Readback, ReadbackComplete},
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
    },
};
use rive_bevy::{RivePlugin, SceneTarget, StateMachine};

const WIDTH: u32 = 256;
const HEIGHT: u32 = 256;
const HUNG_TEST_LIMIT: Duration = Duration::from_secs(600);

fn render_state_machine(format: TextureFormat) -> Vec<u8> {
    let captured: Arc<Mutex<Option<Vec<u8>>>> = Arc::default();
    let captured_in_observer = Arc::clone(&captured);

    let mut app = App::new();
    app.add_plugins(DefaultPlugins).add_plugins(RivePlugin);

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
        format,
        RenderAssetUsages::default(),
    );
    scene_image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::COPY_SRC;
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

    app.world_mut()
        .spawn((Camera2d, RenderTarget::Image(camera_image.into())));

    app.world_mut()
        .spawn(Readback::texture(scene_image.clone()))
        .observe(move |event: On<ReadbackComplete>| {
            let mut captured = captured_in_observer.lock().unwrap();
            if captured.is_none() && event.data.iter().any(|&byte| byte != 0) {
                *captured = Some(event.data.clone());
            }
        });

    let deadline = Instant::now() + HUNG_TEST_LIMIT;
    while captured.lock().unwrap().is_none() && Instant::now() < deadline {
        app.update();
    }

    captured
        .lock()
        .unwrap()
        .take()
        .expect("The Rive scene never produced non-zero pixels in the target image.")
}

fn assert_drawn_artboard(pixels: &[u8]) {
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

    let transparent = pixels.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
    let opaque = pixels.chunks_exact(4).filter(|pixel| pixel[3] == 255).count();
    assert!(
        transparent > 0,
        "The artboard covered every pixel, so the scene was not placed inside the target."
    );
    assert!(
        opaque > 0,
        "No pixel of the target is fully covered, so the artboard drew no solid shape."
    );
}

#[test]
fn renders_a_state_machine_into_an_rgba8_unorm_target() {
    assert_drawn_artboard(&render_state_machine(TextureFormat::Rgba8Unorm));
}

#[test]
fn renders_a_state_machine_into_an_rgba8_unorm_srgb_target() {
    assert_drawn_artboard(&render_state_machine(TextureFormat::Rgba8UnormSrgb));
}
