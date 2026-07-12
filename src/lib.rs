mod assets;
mod components;
pub mod events;
#[cfg(metal_renderer_native)]
mod metal;
#[cfg(metal_renderer_native)]
mod metal_node;
#[cfg(not(metal_renderer_native))]
mod node;
mod plugin;
mod pointer_events;

// Re-export rive-rs
pub use rive_rs;

pub use crate::{
    assets::Riv,
    components::{
        LinearAnimation, MeshEntity, RiveLinearAnimation, RiveStateMachine, SceneImage,
        SceneTarget, SpriteEntity, StateMachine,
    },
    events::GenericEvent,
    plugin::RivePlugin,
    rive_rs::Handle,
};
