use bevy::{asset::VisitAssetDependencies, ecs::system::SystemParam, prelude::*};

use crate::asset::{load_folder_index, FolderIndex};

use super::Arena;

/// A [`Resource`] containing a folder of arenas.
#[derive(Resource, Clone, Debug)]
pub struct ArenaFolder(Handle<FolderIndex>);

impl FromWorld for ArenaFolder {
    fn from_world(world: &mut World) -> Self {
        let handle = world.run_system_cached_with(load_folder_index, "arenas.index".to_string());
        Self(handle.expect("what could possibly go wrong"))
    }
}

/// A [`SystemParam`] for accessing the loaded [`ArenaFolder`].
#[derive(SystemParam)]
pub struct Arenas<'w, 's> {
    folder: Res<'w, ArenaFolder>,
    folder_index: Res<'w, Assets<FolderIndex>>,
    arenas: Res<'w, Assets<Arena>>,
    asset_server: Res<'w, AssetServer>,
    commands: Commands<'w, 's>,
}

impl Arenas<'_, '_> {
    pub fn get(&self) -> Option<impl Iterator<Item = (AssetId<Arena>, &Arena)>> {
        let id = self.folder.0.id();

        if !self.asset_server.is_loaded_with_dependencies(id) {
            // Folder not loaded yet.
            return None;
        }
        let folder = self.folder_index.get(id).unwrap();
        let mut res = vec![];
        folder.visit_dependencies(&mut |id| {
            let id = id.typed::<Arena>();
            let arena = self.arenas.get(id).unwrap();
            res.push((id, arena));
        });
        Some(res.into_iter())
    }
}
