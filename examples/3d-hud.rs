//! Demonstrates a Rive HUD in a 3D scene with a bloom effect.
//! Note that only the `Dashboard` button works at the start of the animation. The other buttons are placeholders.

use std::borrow::Cow;

mod common;

use bevy::{
    light::NotShadowCaster,
    post_process::bloom::{Bloom, BloomCompositeMode, BloomPrefilter},
    prelude::*,
    render::render_resource::Extent3d,
};
use common::close_on_esc;

use rive_bevy::{MeshEntity, RivePlugin, SceneTarget, StateMachine};

fn main() {
    App::new()
        .insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 1.0 / 5.0f32,
            affects_lightmapped_meshes: true,
        })
        .add_plugins(DefaultPlugins)
        .add_plugins(RivePlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, close_on_esc)
        .add_systems(Update, camera_control_system)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(50.0, 50.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.3, 0.3, 0.3))),
        Transform::from_xyz(0.0, -5.0, 0.0),
    ));

    let mut rive_image = Image::default();

    rive_image.resize(Extent3d {
        width: 1920 * 2,
        height: 1080 * 2,
        ..default()
    });

    let rive_image_handle = images.add(rive_image);

    let plane_handle = meshes.add(Rectangle::new(19.20, 10.80));

    let material_handle = materials.add(StandardMaterial {
        base_color_texture: Some(rive_image_handle.clone()),
        reflectance: 1.0,
        perceptual_roughness: 0.0,
        metallic: 0.5,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    let plane_entity = commands
        .spawn((
            Mesh3d(plane_handle),
            MeshMaterial3d(material_handle),
            Transform::from_xyz(0.0, 0.5, 0.05),
        ))
        .id();

    commands
        .spawn(StateMachine {
            riv: asset_server.load("sophia_iii_clear.riv"),
            artboard_handle: rive_rs::Handle::Name(Cow::Owned("SOPHIA III HUD".to_string())),
            ..default()
        })
        .insert(SceneTarget {
            image: rive_image_handle.into(),
            // Adding the mesh here enables mouse input being passed to the Scene.
            mesh: MeshEntity {
                entity: Some(plane_entity),
            },
            ..default()
        });

    // opaque sphere
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(2.0).mesh().ico(3).unwrap())),
        MeshMaterial3d(materials.add(Color::srgb(0.7, 0.2, 0.1))),
        Transform::from_xyz(0.0, 0.5, -5.5),
    ));

    // light
    commands.spawn((
        PointLight {
            intensity: 10000.0,
            range: 40.0,
            // radius: 10.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 4.0).looking_at(Vec3::new(0.0, 0.0, 1.0), Vec3::X),
    ));

    // sky
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::default())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.2, 0.2),
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(200.0)),
        NotShadowCaster,
    ));

    // camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-4.0, 1.0, 15.0).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
        Bloom {
            intensity: 0.2,
            low_frequency_boost: 0.7,
            low_frequency_boost_curvature: 0.95,
            high_pass_frequency: 1.0,
            prefilter: BloomPrefilter {
                threshold: 0.6,
                threshold_softness: 0.2,
            },
            composite_mode: BloomCompositeMode::Additive,
            ..default()
        },
    ));
}

fn camera_control_system(
    mut camera: Query<&mut Transform, With<Camera3d>>,
    time: Res<Time>,
    input: Res<ButtonInput<KeyCode>>,
) {
    let Ok(mut camera_transform) = camera.single_mut() else {
        return;
    };

    let rotation = if input.pressed(KeyCode::ArrowLeft) {
        time.delta_secs()
    } else if input.pressed(KeyCode::ArrowRight) {
        -time.delta_secs()
    } else {
        0.0
    };

    let movement = if input.pressed(KeyCode::ArrowUp) {
        -time.delta_secs()
    } else if input.pressed(KeyCode::ArrowDown) {
        time.delta_secs()
    } else {
        0.0
    };

    camera_transform.rotate_around(Vec3::ZERO, Quat::from_rotation_y(rotation));
    camera_transform.translation += Vec3::new(0.0, 0.0, movement * 10.0);
}
