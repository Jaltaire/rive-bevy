//! An example showcasing how to receive events from a Rive state machine.

mod common;

use bevy::{prelude::*, render::render_resource::Extent3d};
use common::close_on_esc;
use rive_bevy::{
    rive_rs::state_machine::Property, GenericEvent, RivePlugin, SceneTarget, SpriteEntity,
    StateMachine,
};

#[derive(Component)]
struct RatingText;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin::default()))
        .add_plugins(RivePlugin)
        .add_systems(Startup, (setup_animation, setup_text))
        .add_systems(Update, close_on_esc)
        .add_systems(Update, receive_rive_events_system)
        .run();
}

fn setup_animation(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
) {
    let mut animation_image = Image::default();

    animation_image.resize(Extent3d {
        width: 1024,
        height: 1024,
        ..default()
    });

    let animation_image_handle = images.add(animation_image.clone());

    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            ..default()
        },
    ));

    let sprite_entity = commands
        .spawn((
            Sprite::from_image(animation_image_handle.clone()),
            Transform::from_scale(Vec3::splat(0.5)).with_translation(Vec3::new(0.0, 0.0, 0.0)),
        ))
        .id();

    let state_machine = StateMachine {
        riv: asset_server.load("rating_animation.riv"),
        // Optionally provide State Machine name to load
        // handle: rive_bevy::Handle::Name(Cow::Owned("State Machine 1".to_string())),
        // Optionally provide artboard name to load
        // artboard_handle: rive_bevy::Handle::Name(Cow::Owned("New Artboard".to_string())),
        ..default()
    };

    commands.spawn(state_machine).insert(SceneTarget {
        image: animation_image_handle.into(),
        sprite: SpriteEntity {
            entity: Some(sprite_entity),
        },
        ..default()
    });
}

fn setup_text(mut commands: Commands) {
    commands
        .spawn((
            Text::new("Rating: "),
            TextFont {
                font_size: FontSize::Px(32.0),
                ..default()
            },
            TextColor(Color::BLACK),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                left: Val::Px(10.0),
                ..default()
            },
        ))
        .with_child((TextSpan::default(), RatingText));
}

fn receive_rive_events_system(
    mut rive_event: MessageReader<GenericEvent>,
    mut text_query: Query<&mut TextSpan, With<RatingText>>,
) {
    for event in rive_event.read() {
        info!("Rive event: {:?}", event);
        // We can match on the event name and extract the properties.
        if event.name == "Star" {
            // Find the "rating" property which is a Property::Number.
            if let Some(Property::Number(rating)) = event.properties.get("rating") {
                info!("Rating: {:?}", rating);

                let Ok(mut text) = text_query.single_mut() else {
                    return;
                };
                text.0 = rating.to_string();
            }
        }
    }
}
