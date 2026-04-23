//! An example showcasing how to manipulate Rive state machine inputs and text.
//! Community animation file: https://rive.app/community/5649-11315-circle-fui/

mod common;

use bevy::{
    input::{keyboard::KeyboardInput, ButtonState},
    prelude::*,
    render::render_resource::Extent3d,
};
use common::close_on_esc;
use rive_bevy::{
    events::{self, InputValue},
    rive_rs::components::TextValueRun,
    RivePlugin, RiveStateMachine, SceneTarget, SpriteEntity, StateMachine,
};

const BACKGROUND_COLOR: Color = Color::srgb(0.0, 0.0, 0.0);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin::default()))
        .add_plugins(RivePlugin)
        .insert_resource(ClearColor(BACKGROUND_COLOR))
        .add_systems(Startup, (setup_animation, setup_text))
        .add_systems(Update, close_on_esc)
        .add_systems(
            Update,
            (update_state_machine_system, update_rive_text_system),
        )
        .run();
}

fn setup_animation(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
) {
    let mut animation_image = Image::default();

    animation_image.resize(Extent3d {
        width: 2000,
        height: 1700,
        ..default()
    });

    let animation_image_handle = images.add(animation_image.clone());

    commands.spawn(Camera2d);

    let sprite_entity = commands
        .spawn((
            Sprite::from_image(animation_image_handle.clone()),
            Transform::from_scale(Vec3::splat(0.5)).with_translation(Vec3::new(0.0, 0.0, 0.0)),
        ))
        .id();

    let state_machine = StateMachine {
        riv: asset_server.load("circle-fui.riv"),
        // artboard_handle: rive_bevy::Handle::Name("StateMachine".into()), // specify the artboard by name
        ..default()
    };

    commands.spawn(state_machine).insert(SceneTarget {
        image: animation_image_handle.into(),
        // Adding the sprite here enables mouse input being passed to the Scene.
        sprite: SpriteEntity {
            entity: Some(sprite_entity),
        },
        ..default()
    });
}

fn setup_text(mut commands: Commands) {
    commands.spawn((
        Text::new("Update Rive state machine inputs and text"),
        TextFont {
            font_size: 22.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
    ));

    commands.spawn((
        Text::new("Press `Return` to toggle, then type to change text..."),
        TextFont {
            font_size: 22.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(52.0),
            left: Val::Px(10.0),
            ..default()
        },
    ));
}

fn update_state_machine_system(
    kbd: Res<ButtonInput<KeyCode>>,
    mut query: Query<(Entity, &mut RiveStateMachine)>,
    mut input_events: MessageWriter<events::Input>,
) {
    if kbd.just_pressed(KeyCode::Enter) {
        // Get the State Machine and its Entity
        let Ok((entity, state_machine)) = query.single_mut() else {
            return;
        };

        // Read the current value of an input
        let center_hover_current = state_machine.get_bool("centerHover").unwrap().get();

        // Send a new value to the input using Bevy events.
        {
            input_events.write(events::Input {
                state_machine: entity,
                name: "centerHover".into(),
                value: InputValue::Bool(!center_hover_current),
            });
        }

        // Alternatively we can use the raw API and send the value directly to the Rive C++ API.
        // Comment the above Bevy event and uncomment the below.
        {
            // state_machine
            //     .get_bool("centerHover")
            //     .unwrap()
            //     .set(!center_hover_current);
        }
    }
}

fn update_rive_text_system(
    kbd: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut RiveStateMachine>,
    mut string: Local<String>,
    mut keyboard_input: MessageReader<KeyboardInput>,
) {
    // On toggle, clear the string.
    if kbd.just_pressed(KeyCode::Enter) {
        string.clear();
        return;
    }

    let mut did_change = false;
    if kbd.just_pressed(KeyCode::Backspace) {
        did_change = true;
        string.pop();
    }
    for ev in keyboard_input.read() {
        if ev.state == ButtonState::Pressed {
            if let Some(text) = &ev.text {
                for char in text.chars() {
                    if !char.is_control() {
                        string.push(char);
                        did_change = true;
                        info!("{}", string.as_str());
                    }
                }
            }
        }
    }

    // Update our Rive text if the string changed.
    if did_change {
        let Ok(state_machine) = query.single_mut() else {
            return;
        };

        if !state_machine.get_bool("centerHover").unwrap().get() {
            return;
        }

        let mut artboard = state_machine.artboard();

        let mut text: TextValueRun = artboard
            .components()
            .find(|comp| comp.name() == "Sector")
            .unwrap()
            .try_into()
            .unwrap();

        let mut formatted_value: String = string.to_owned();
        formatted_value.push_str(" : ");

        text.set_text(&formatted_value);
    }
}
