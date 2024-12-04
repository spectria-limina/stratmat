use std::{io, marker::PhantomData};

use bevy::{
    asset::AssetLoader,
    prelude::*,
    tasks::futures_lite::{io::BufReader, AsyncBufReadExt, StreamExt},
};

#[derive(Asset, TypePath)]
pub struct AssetListing<A: Asset> {
    #[dependency]
    handles: Vec<Handle<A>>,
}

#[derive(Copy, Clone, Debug)]
pub struct ListingLoader<A>(PhantomData<A>);

impl<A> Default for ListingLoader<A> {
    fn default() -> Self {
        Self(default())
    }
}

impl<A: Asset> AssetLoader for ListingLoader<A> {
    type Asset = AssetListing<A>;
    type Settings = ();
    type Error = io::Error;

    async fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        _settings: &Self::Settings,
        load_context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut lines = BufReader::new(reader).lines();
        let mut handles = vec![];
        while let Some(line) = lines.next().await {
            handles.push(load_context.load(line?));
        }
        Ok(AssetListing { handles })
    }

    fn extensions(&self) -> &[&str] {
        &["listing"]
    }
}

pub trait ListingExt {
    fn init_asset_listing<A: Asset>(&mut self) -> &mut Self;
}
impl ListingExt for App {
    fn init_asset_listing<A: Asset>(&mut self) -> &mut Self {
        self.init_asset::<AssetListing<A>>()
            .init_asset_loader::<ListingLoader<A>>()
    }
}
