use std::string::FromUtf8Error;
use std::sync::{Arc, Mutex};

use bevy::asset::processor::LoadTransformAndSave;
use bevy::asset::saver::AssetSaver;
use bevy::asset::transformer::IdentityAssetTransformer;
use bevy::asset::{AssetLoader, AsyncWriteExt, LoadDirectError, LoadedFolder, LoadedUntypedAsset};
use bevy::prelude::*;
use bevy::tasks::futures_lite::io::BufReader;
use bevy::tasks::futures_lite::{io, AsyncBufReadExt, FutureExt, StreamExt};
use bevy::tasks::poll_once;
use bevy::utils::HashMap;
use thiserror::Error;

use crate::ecs::{trigger_all_events, AssetCommandsExt};
use crate::future::OnceTardis;

#[derive(Asset, Clone, Debug, TypePath)]
pub struct FolderIndex {
    #[dependency]
    pub handles: Vec<UntypedHandle>,
}

// This is the most disgusting, dirty hack.
//
// Important: This comment makes more sense, and is probably even funnier, if you
// read it AFTER the one in IndexLoader. IndexLoader is cursed. But
// IndexPseudoLoader is an eldritch horror.
//
// We want to get at the asset on the AssetServer. But we don't have access to it.
// We can't try to load the folder. That will fail because it's a directory.
//
// But if we ask for a deferred load, that has to hit the AssetServer,
// and the asset server will check its cache and, if the asset is fully loaded,
// return it without spawning a new loading task.
//
// However, that actually puts us in a bit of a pickle, as deferring the load means
// that we can't get at the LoadedFolder. Because it's not actually loaded. So what
// does this mean? Is it all over for the intrepid Spaceman Spiff?
//
// His attempts so far to defeat the Zargon asset loader have only met dismal failure.
// What other options does he have left? Things look bleak.
//
// There's no chance if he tries an immediate load. Even if he stuffs the immediate
// loader with a no-op reader and a no-op loader, to sneak a directory through the
// Zargon security system without ever looking at it, the hideous Zondarg at the exit
// will force him to give up a LoadedFolder, and if he lies, she will stuff it right
// into her cache and all will be lost.
//
// Too late, Spaceman Spiff discovers his mistake. The asset server does not hold the
// loaded assets, most importantly the directory listing. It never did. His entire
// expedition was a futile endeavour, and now he must find a way to liberate enough
// spare parts from the Zargons to escape this wretched planet.
//
// But wait! There's a smuggling route he overlooked! That's right... the temporal
// transportalizer! He doesn't need to steal the data from the Zargons at all, because
// he had it all along! The Zargons tried to hide the transportalizer by calling it
// "Settings", but their little tricks are no match for Spaceman Spiff's cunning.
//
// Still, the route is difficult. So difficult, in fact, that it requires temporal
// manipulation. Yes, our brave Spaceman Spiff will need to take a temporal anomaly
// with him and successfully navigate the time currents to pull of his perfect heist.
//
// Just when Spiff thought he was in the clear, he saw his data disappear before his
// very eyes! The last obstacle between him and freedom was the dreaded serializer,
// and there's no way his temporal anomaly can make it through the serializer's
// cryogenic preservation field intact.
//
// What can he do?  Is this the end?
//
// As if! The serializer is Bloatoid cryogenic technology, easily fooled. He hides
// the box of handles he's carrying behind a large bould and finds a shimmering,
// handle-sized rock nearby. With the unmatched aim of a marksman, he tosses the rock
// behind the serializer as hard as he can. The decoy works perfectly, and Spiff has
// no more obstacles on his escape.
//
// Still, it took Spiff a long time to complete his temporal and relativistic dimensional
// infrastructure system. But he displayed remarkable, even uncharacteristic patience,
// ignoring the Yorblax crowded around him trying to scare him away, muttering something
// about "rules". Finally, with his work complete, Spiff pressed the big red button...
//
// Just when Spiff thought he was in the clear, he saw his data disappear before his
// very eyes! The last obstacle between him and freedom was the dreaded---no! It can't be!
// It's a reentrancy time loop!
//
// The intrepid explorer knew he had to act fast, or he could be caught in the loop
// forever. Or worse, he could be smashed against stack protector or suffer an existensial
// fault. He had no time to lose. Clearly it was the manner of his loading that was the
// problem... the Zargons had found the indexing beacon, the start of Spiff's carefully
// honed Rube Goldberg machine, and in their confused flailing tossed it into the temporal
// anomaly. What could be done? Was there any satisfactory option remaining?
//
// Belatedly, Spiff realized that perhaps he didn't need to go through all this effort...
// Perhaps he could just have cut off the Zargons' power source and that would have been
// that. But now it was too late. But now it was too late. But now it was---by the Great
// Quazon, it's getting bad! There's no time to lose!
//
// But wait... could the solution truly be so simple?
//
// ... ... many hours pass ...
//
// The solution could not, unfortunately, prove so simple. But Spaceman Spiff undaunted,
// perservered. He found a hole in the Zargonian armour: in the right circumstances, the
// Garflonian guards would see him trying to escape with a handle and throw an error, but
// the lazy Smitraxian overseers would let it slip.
//
// Thanks to this oversight, and making the best of his dashing ingenuity and scientific
// know-how, Spaceman Spiff built a gateway to a pocket dimension which, through careful
// navigation of labyrinthine temporal anomalies with infinitely replicating time
// machines, the undaunted Spaceman Spiff finally escaped the Dread Planet of Asset.
//
// ... but at what cost?
//
// Flying free in his hastily-repaired ship, zipping through the dark, empty reaches of
// interplanetary space, Spaceman Spiff last track of time. And so he didn't notice the
// consequences of his temporal tomfoolery: that everything was, every so slowly,
// slowing down. Caught in the throes of an interstellar deadlock, because of the
// folder loader getting blocked on asset processing which was waiting on the folder to
// finish loading, what could he possibly do to escape?

#[derive(Clone, Default, Resource)]
struct TardisFleet(Arc<Mutex<HashMap<String, OnceTardis<Vec<UntypedHandle>>>>>);

#[derive(Clone)]
pub struct IndexPseudoLoader {
    fleet: TardisFleet,
}

impl AssetLoader for IndexPseudoLoader {
    type Asset = FolderIndex;
    type Settings = ();
    type Error = IndexPseudoLoaderError;

    async fn load(
        &self,
        _reader: &mut dyn bevy::asset::io::Reader,
        _settings: &Self::Settings,
        load_context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let file = load_context.asset_path().to_string();
        let folder = file.strip_suffix(".index").unwrap();
        // This likes to throw errors, but it's okay.
        // It establishes the dependency anyway.
        load_context.load::<LoadedFolder>(folder);
        let tardis = (|| -> Result<_, IndexPseudoLoaderError> {
            let mut bad_wolf = self
                .fleet
                .0
                .lock()
                .map_err(|_| IndexPseudoLoaderError::TimeyWimey)?;
            Ok(bad_wolf.entry(file.clone()).or_default().clone())
        })()?;
        info!("phone box '{}' awaiting contraband", file);
        if let Some(handles) = poll_once(&tardis).await {
            Ok(FolderIndex { handles })
        } else {
            Err(IndexPseudoLoaderError::Stasis)
        }
    }

    fn extensions(&self) -> &[&str] {
        &["index"]
    }
}

#[derive(Error, Debug)]
pub enum IndexPseudoLoaderError {
    #[error("the space-time continuum has been damaged beyond all repair!")]
    TimeyWimey,
    #[error("everyone is frozen in time")]
    Stasis,
}

// Don't blink.
// Don't even blink.
// Blink and your asset is dead.
#[derive(Default, Clone, Resource, Debug)]
pub struct StatueGarden<T>(Vec<T>);

pub fn load_folder_index(
    In(file): In<String>,
    server: Res<AssetServer>,
    mut commands: Commands,
    mut garden: ResMut<StatueGarden<Handle<LoadedFolder>>>,
) -> Handle<FolderIndex> {
    let handle = server.load::<FolderIndex>(&file);
    if cfg!(target_arch = "wasm32") {
        return handle;
    }
    let folder = file.strip_suffix(".index").expect("so tired");
    info!("scouting for contraband for '{}' in '{}'", file, folder);
    let folder_handle = server.load_folder(folder);
    let asset_id = folder_handle.id();
    garden.0.push(folder_handle);
    commands.run_system_when_asset_loaded_with(
        asset_id,
        move |In((asset_id, file)): In<(AssetId<LoadedFolder>, String)>,
              folders: Res<Assets<LoadedFolder>>,
              fleet: Res<TardisFleet>| {
            let folder = folders
                .get(asset_id)
                .expect("our portal should keep the asset in tact");
            // Allons-y!
            let mut boxen = fleet.0.lock().expect("no time collapse today please :(");
            let phone_box = boxen.entry(file.clone()).or_default();
            phone_box.set(folder.handles.clone());
            info!("phone box '{}' loaded with contraband", file)
        },
        (asset_id, file),
    );
    handle
}

/// Saves a LoadedFolder as list of its contents in a NUL-separated list of paths.
#[derive(Debug, Copy, Clone, Default)]
pub struct IndexSaver;

impl AssetSaver for IndexSaver {
    type Asset = FolderIndex;
    type Settings = ();
    type OutputLoader = IndexLoader;
    type Error = IndexSaverError;

    async fn save(
        &self,
        writer: &mut bevy::asset::io::Writer,
        asset: bevy::asset::saver::SavedAsset<'_, Self::Asset>,
        _settings: &Self::Settings,
    ) -> Result<<Self::OutputLoader as AssetLoader>::Settings, Self::Error> {
        for handle in &asset.handles {
            let path = handle.path().ok_or(IndexSaverError::NoPath)?.to_string();
            writer.write_all(path.as_bytes()).await?;
            writer.write_all(b"\0").await?;
        }
        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum IndexSaverError {
    #[error("no path to asset")]
    NoPath,
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

#[derive(Debug, Copy, Clone, Default)]
pub struct IndexLoader;

impl AssetLoader for IndexLoader {
    type Asset = FolderIndex;
    type Settings = ();
    type Error = IndexLoaderError;

    async fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        _settings: &Self::Settings,
        load_context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut paths = BufReader::new(reader).split(0);
        let mut handles = vec![];
        while let Some(path) = paths.next().await {
            let path = String::from_utf8(path?)?;
            // Okay so this is majorly cursed.
            //
            // We are in an async context, so we are free to "immediately" load all the
            // assets in the folder, which will cause our operation to not fully complete
            // until they're all loaded, but that's fine because the caller can deal with
            // that (e.g. by loading this folder async in the background).
            //
            // However, if we use NestedLoader::immediate(), we immediately get handed a
            // ErasedLoadedAsset, which is a very useless type to us. To actually produce a
            // LoadedFolder, we need a list of UntypedHandles. And without direct access to
            // the AssetServer, we can't just generate a new UntypedHandle from the path.
            //
            // So our only choice is to use NestedLoader::deferred() instead. This, however,
            // does not actually start loading the asset. It instead gives a
            // LoadedUntypedAsset, a misnomer if I ever saw one, which is a Handle to the
            // UntypedHandle that will be created if anyone ever bothers trying to actually
            // load the asset. I have no idea why this double indirection is necessary.
            // Maybe something to do with async. I'm really not sure.
            //
            // This LoadedUntypedAsset is not, however, useless. It is a pathway to an
            // UntypedHandle, and the key to unlock it is to attempt to load the
            // LoadedUntypedAsset.  However, this time we actually *do* want to "block" (by
            // which I mean await on) the loading so that we can use it to make our nice
            // UntypedHandle. Thus, we need to load it in immediate mode.
            //
            // And just our luck, when you load an asset in immediate mode with a NestedLoader,
            // that is the one case, outside asset processing, where end user code can actually
            // cause an asset to be loaded without its dependencies.
            //
            // So at least we know that this won't cause the normal dependency loading process
            // to make us finish loading the underlying assets before we return. But will some
            // other part of the sprawling, incomprehensible network of futures cause just such
            // a thing to happen?
            //
            // we may never know
            let loader = load_context.loader().with_unknown_type().deferred();
            let outer_handle = loader.load(path);
            let loader = load_context.loader().with_static_type().immediate();
            let handle = loader
                .load::<LoadedUntypedAsset>(
                    outer_handle
                        .path()
                        .expect("we just loaded this asset it better have a path"),
                )
                // Fun fact: the result of this await is a LoadedAsset<LoadedUntypedAsset>.
                .await?
                .take()
                .handle;
            handles.push(handle);
        }
        Ok(FolderIndex { handles })
    }
}

#[derive(Error, Debug)]
pub enum IndexLoaderError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("non-UTF-8 asset path: {}", String::from_utf8_lossy(.0.as_bytes()))]
    NonUtf8Path(#[from] FromUtf8Error),
    #[error("loading asset or maybe loading something in context idk: {0}")]
    Load(#[from] LoadDirectError),
}

pub type FolderIndexer =
    LoadTransformAndSave<IndexPseudoLoader, IdentityAssetTransformer<FolderIndex>, IndexSaver>;

pub struct FolderIndexingPlugin;

impl Plugin for FolderIndexingPlugin {
    fn build(&self, app: &mut App) {
        let fleet = TardisFleet::default();
        app.init_asset::<FolderIndex>()
            .insert_resource(fleet.clone())
            .init_resource::<StatueGarden<Handle<LoadedFolder>>>()
            .register_asset_loader(IndexPseudoLoader { fleet })
            // .register_asset_loader(IndexLoader)
            .register_asset_processor(FolderIndexer::new(default(), default()))
            //.set_default_asset_processor::<FolderIndexer>("index")
            .add_systems(PreUpdate, trigger_all_events::<AssetEvent<LoadedFolder>>);
    }
}
