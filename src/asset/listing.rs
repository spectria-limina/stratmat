use std::{io, marker::PhantomData, path::Path};

use bevy::{
    asset::{AssetLoader, VisitAssetDependencies},
    prelude::*,
};
use thiserror::Error;

use super::lifecycle::LifecycleExts;

#[derive(TypePath, Clone)]
pub struct AssetListing<A: Asset> {
    pub name: String,
    pub contents: Vec<Handle<A>>,
    pub subdirs: Vec<AssetListing<A>>,
}
impl<A: Asset> AssetListing<A> {
    pub fn get_all<'a>(
        &self,
        asset_server: &AssetServer,
        assets: &'a Assets<A>,
    ) -> impl Iterator<Item = (Handle<A>, &'a A)> {
        let mut res = vec![];
        self.visit_dependencies(&mut |id| {
            let id = id.typed::<A>();
            let handle = asset_server.get_id_handle(id).unwrap();
            if let Some(asset) = assets.get(id) {
                res.push((handle, asset));
            } else {
                warn!(
                    "AssetListing::<{}>::get_all skipped {:?} because it is not yet loaded",
                    std::any::type_name::<Self>(),
                    handle
                );
            }
        });
        res.into_iter()
    }

    fn load_from_tataru(
        listing: tataru::Listing,
        path: &Path,
        load_context: &mut bevy::asset::LoadContext<'_>,
    ) -> AssetListing<A> {
        Self {
            name: listing.name,
            contents: listing
                .contents
                .into_iter()
                .map(|name| {
                    let path = path.join(name);
                    debug!("Loading listing asset {}", path.display());
                    load_context.load(path)
                })
                .collect(),
            subdirs: listing
                .subdirs
                .into_iter()
                .map(|(name, subdir)| {
                    Self::load_from_tataru(subdir, &path.join(name), load_context)
                })
                .collect(),
        }
    }
}

impl<A: Asset> Asset for AssetListing<A> {}

impl<A: Asset> VisitAssetDependencies for AssetListing<A> {
    fn visit_dependencies(&self, visit: &mut impl FnMut(bevy::asset::UntypedAssetId)) {
        for handle in &self.contents {
            visit(handle.id().untyped());
        }
        for listing in &self.subdirs {
            listing.visit_dependencies(visit);
        }
    }
}

#[derive(Copy, Clone, derive_more::Debug)]
pub struct ListingLoader<A>(#[debug(skip)] PhantomData<A>);

impl<A> Default for ListingLoader<A> {
    fn default() -> Self {
        Self(default())
    }
}

impl<A: Asset> AssetLoader for ListingLoader<A> {
    type Asset = AssetListing<A>;
    type Settings = ();
    type Error = ListingLoadError;

    async fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        _settings: &Self::Settings,
        load_context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut buf = vec![];
        reader.read_to_end(&mut buf).await?;
        let listing: tataru::Listing = serde_json::from_slice(&buf)?;
        debug!(
            "Loaded {} listing: {:?}",
            std::any::type_name::<A>(),
            listing
        );
        Ok(AssetListing::load_from_tataru(
            listing,
            #[allow(clippy::unnecessary_to_owned)]
            &load_context
                .asset_path()
                .path()
                .parent()
                .expect("a file path must have a parent")
                .to_owned(),
            load_context,
        ))
    }

    fn extensions(&self) -> &[&str] {
        &["listing"]
    }
}

#[derive(Error, Debug)]
pub enum ListingLoadError {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("Unabled to deserialize: {0}")]
    Deserialize(#[from] serde_json::Error),
}

pub trait ListingExt {
    fn init_asset_listing<A: Asset>(&mut self) -> &mut Self;
}
impl ListingExt for App {
    fn init_asset_listing<A: Asset>(&mut self) -> &mut Self {
        self.init_asset_with_lifecycle::<AssetListing<A>>()
            .init_asset_loader::<ListingLoader<A>>()
    }
}
