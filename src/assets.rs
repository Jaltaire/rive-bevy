use bevy::{
    asset::{io::Reader, AssetLoader, LoadContext},
    prelude::*,
    reflect::TypePath,
};
use rive_rs::File;
use thiserror::Error;

#[derive(Asset, Debug, Deref, TypePath)]
pub struct Riv(pub rive_rs::File);

#[derive(Debug, Error)]
pub enum RivLoaderError {
    #[error("Could not load Riv: {0}.")]
    Io(#[from] std::io::Error),
    #[error("Could not read Riv: {0}.")]
    RivError(#[from] rive_rs::Error),
}

#[derive(Default, TypePath)]
pub struct RivLoader;

impl AssetLoader for RivLoader {
    type Asset = Riv;
    type Settings = ();
    type Error = RivLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        Ok(Riv(File::new(&bytes)?))
    }

    fn extensions(&self) -> &[&str] {
        &["riv"]
    }
}
