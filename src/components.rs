use std::sync::Arc;

use bevy::{
    prelude::*,
    render::{extract_component::ExtractComponent, sync_component::SyncComponent},
};

use crate::Riv;

#[derive(Clone, Component, Debug, Default)]
pub struct LinearAnimation {
    pub riv: Handle<Riv>,
    pub artboard_handle: rive_rs::Handle,
    pub handle: rive_rs::Handle,
    pub sprite_entity: Option<Entity>,
}

#[derive(Component, Debug, Deref, DerefMut)]
pub struct RiveLinearAnimation(pub rive_rs::LinearAnimation);

#[derive(Clone, Component, Debug, Default)]
pub struct StateMachine {
    pub riv: Handle<Riv>,
    pub artboard_handle: rive_rs::Handle,
    pub handle: rive_rs::Handle,
    pub sprite_entity: Option<Entity>,
}

#[derive(Component, Debug, Deref, DerefMut)]
pub struct RiveStateMachine(pub rive_rs::StateMachine);

#[derive(Component, Debug)]
pub(crate) struct MissingArtboard;

#[derive(Component, Debug)]
pub(crate) struct MissingLinearAnimation;

#[derive(Component, Debug)]
pub(crate) struct MissingStateMachine;

#[derive(Clone, Component, Debug, Default, Deref, DerefMut)]
pub struct Viewport(pub rive_rs::Viewport);

#[derive(Clone, Component, Debug, Default, Deref)]
pub struct MeshEntity {
    pub entity: Option<Entity>,
}

#[derive(Clone, Component, Debug, Default, Deref)]
pub struct SpriteEntity {
    pub entity: Option<Entity>,
}

#[derive(Bundle, Debug, Default)]
pub struct SceneTarget {
    pub image: SceneImage,
    pub sprite: SpriteEntity,
    pub mesh: MeshEntity,
}

#[derive(Clone, Component, Debug, Default, Deref, DerefMut)]
pub struct SceneImage(pub Handle<Image>);

impl From<Handle<Image>> for SceneImage {
    fn from(handle: Handle<Image>) -> Self {
        Self(handle)
    }
}

#[derive(Component, Deref)]
pub(crate) struct VelloFragment(pub Arc<vello::Scene>);

#[derive(Component)]
pub(crate) struct VelloScene {
    pub fragment: Arc<vello::Scene>,
    pub image_handle: Handle<Image>,
    pub width: u32,
    pub height: u32,
}

impl SyncComponent for VelloFragment {
    type Target = VelloScene;
}

impl ExtractComponent for VelloFragment {
    type QueryData = (
        &'static VelloFragment,
        &'static SceneImage,
        &'static Viewport,
    );

    type QueryFilter = ();

    type Out = VelloScene;

    fn extract_component(
        (fragment, image, viewport): bevy::ecs::query::QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out> {
        Some(VelloScene {
            fragment: fragment.0.clone(),
            image_handle: image.0.clone(),
            width: viewport.width(),
            height: viewport.height(),
        })
    }
}
