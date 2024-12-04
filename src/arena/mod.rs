use std::{
    io,
    path::{Path, PathBuf},
};

use avian2d::prelude::*;
use bevy::{
    asset::{AssetLoader, ParseAssetPathError, VisitAssetDependencies},
    ecs::system::{SystemParam, SystemState},
    prelude::*,
    render::camera::ScalingMode,
};
use bevy_inspector_egui::InspectorOptions;
use itertools::Itertools;
use serde::Deserialize;
use thiserror::Error;

use crate::{
    asset::{
        lifecycle::{AssetHookExt, InitExt},
        listing::{AssetListing, ListingExt},
    },
    waymark::Waymark,
    Layer,
};

pub mod menu;

/// The file extension of `Arena` files.
const EXTENSION: &str = "arena.ron";
/// The path, relative to the assets directory, to the directory where `Arena` files are stored.
const DIR: &str = "arenas";

/// Get the asset path for an arena, given its path minus the
/// constant directory and extension parts.
pub fn asset_path(arena: impl AsRef<Path>) -> PathBuf {
    let mut path = PathBuf::new();
    path.push(DIR);
    path.push(arena);
    path.set_extension(EXTENSION);
    path
}

/// Quick and dirty enum to support arena collision shapes until something better comes along.
///
/// TODO: is there any hope of us ever eliminating this kind of type?
#[derive(Reflect, Copy, Clone, PartialEq, Debug, Deserialize)]
pub enum ArenaShape {
    Rect(f32, f32),
    Circle(f32),
}

impl From<ArenaShape> for Collider {
    fn from(value: ArenaShape) -> Self {
        match value {
            ArenaShape::Rect(width, height) => Collider::rectangle(width, height),
            ArenaShape::Circle(radius) => Collider::circle(radius),
        }
    }
}

/// An [`Arena`] is the backdrop to a fight, and includes everything needed to stage and set up a fight,
/// such as the arena's background image, dimensions, and other metadata.
#[derive(Asset, Reflect, Clone, Debug, Deserialize, InspectorOptions)]
pub struct Arena {
    pub name: String,
    pub short_name: String,
    /// The FFXIV map ID.
    pub map_id: u32,
    /// The asset path to the background image.
    ///
    /// In an actual asset file, this should be specified as a relative path from the asset,
    /// or an absolute path from the asset root.
    /// It will be replaced with the correct full asset path during loading.
    pub background_path: String,
    /// The size of the arena image, in yalms.
    pub size: Vec2,
    /// The in-game coordinates of the center of the arena, in yalms.
    /// The Y-coordinate in stratmat corresponds to the Z-coordinate in FFXIV.
    /// Used for import/export only; internally the origin is always the center.
    pub offset: Vec2,
    /// The shape of the actual usuable arena surface, inside the (death)wall.
    pub shape: ArenaShape,
}

#[derive(Default, Copy, Clone, Debug)]
pub struct ArenaLoader;

#[derive(Error, Debug)]
pub enum ArenaLoadError {
    #[error("Could not load asset file: {0}")]
    Io(#[from] io::Error),
    #[error("Could not parse asset file: {0}")]
    Parse(#[from] ron::error::SpannedError),
    #[error("Invalid image path in arena asset: {0}")]
    ImagePath(#[from] ParseAssetPathError),
}

impl AssetLoader for ArenaLoader {
    type Asset = Arena;
    type Settings = ();
    type Error = ArenaLoadError;

    async fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        _settings: &Self::Settings,
        load_context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).await?;
        let mut data: Arena = ron::de::from_bytes(&buf)?;
        data.background_path = load_context
            .asset_path()
            .resolve(&data.background_path)?
            .to_string();
        Ok(data)
    }

    fn extensions(&self) -> &[&str] {
        &[EXTENSION]
    }
}

/// Marker component for the current arena background.
///
/// Currently only one is allowed at a time.
///
/// TODO: Make more than one allowed at a time.
#[derive(Component, Reflect, Clone, Debug)]
pub struct ArenaBackground {
    pub handle: Handle<Arena>,
}

/// How big the viewport should be relative to the size of the arena.
const ARENA_VIEWPORT_SCALE: f32 = 1.1;

/// Z-coordinate of the arena background.
///
/// Our default viewport is (-1000.0, 1000.0), so make sure we are ever so slightly inside that.
const ARENA_BACKGROUND_Z: f32 = -999.0;

/// Bundle of components for the arena background.
#[derive(Bundle)]
pub struct ArenaBackgroundBundle {
    name: Name,
    sprite: Sprite,
    transform: Transform,
    collider: Collider,
    layers: CollisionLayers,
    pickable: PickingBehavior,
}

impl ArenaBackgroundBundle {
    pub fn new(arena: &Arena, image: Handle<Image>) -> Self {
        Self {
            name: format!("{} Background", arena.short_name).into(),
            sprite: Sprite {
                image,
                custom_size: Some(arena.size),
                ..default()
            },
            transform: Transform::from_xyz(0.0, 0.0, ARENA_BACKGROUND_Z),
            collider: arena.shape.into(),
            layers: CollisionLayers::new([Layer::DragSurface], [Layer::Dragged]),
            pickable: PickingBehavior::IGNORE,
        }
    }
}

/// Spawn an arena entity.
///
/// This will defer most of the work until the arena is loaded.
pub fn spawn_arena(In(handle): In<Handle<Arena>>, world: &mut World) {
    let path = world.resource::<AssetServer>().get_path(&handle);
    debug!(
        "spawning new arena with asset ID {} from path '{path:?}'",
        handle.id()
    );
    let id = world
        .spawn(ArenaBackground {
            handle: handle.clone(),
        })
        .id();
    world.on_asset_loaded_with(&handle, finish_spawn_arena, id);
}

#[derive(SystemParam)]
struct ArenaSpawnState<'w, 's> {
    arena_q: Query<'w, 's, &'static ArenaBackground>,
    camera_q: Query<'w, 's, &'static mut OrthographicProjection, With<Camera2d>>,
    arenas: Res<'w, Assets<Arena>>,
    asset_server: Res<'w, AssetServer>,
}

#[derive(Resource)]
struct CachedArenaSpawnState(SystemState<ArenaSpawnState<'static, 'static>>);

impl FromWorld for CachedArenaSpawnState {
    fn from_world(world: &mut World) -> Self {
        Self(SystemState::new(world))
    }
}

/// Finish the post-asset-load spawning of an arena.
fn finish_spawn_arena(In(id): In<Entity>, world: &mut World) {
    world.resource_scope(|world, mut state: Mut<CachedArenaSpawnState>| {
        let ArenaSpawnState {
            arena_q,
            mut camera_q,
            arenas,
            asset_server,
        } = state.0.get_mut(world);

        let Ok(background) = arena_q.get(id) else {
            // The entity was despawned or the ArenaBackground removed, so abort.
            return;
        };
        let Some(arena) = arenas.get(&background.handle) else {
            warn!("finish_spawn_arena called with asset not loaded!");
            return;
        };
        // FIXME: Single-camera assumption.
        camera_q.single_mut().scaling_mode = ScalingMode::AutoMin {
            min_width: arena.size.x * ARENA_VIEWPORT_SCALE,
            min_height: arena.size.y * ARENA_VIEWPORT_SCALE,
        };
        let background = asset_server.load(&arena.background_path);
        let bundle = ArenaBackgroundBundle::new(arena, background);
        world.entity_mut(id).insert(bundle);
    });
}

/// Despawn all arenas.
pub fn despawn_all_arenas(world: &mut World) {
    let mut q = world.query_filtered::<Entity, With<ArenaBackground>>();
    for id in q.iter(world).collect_vec() {
        world.despawn(id);
    }
}

/// A [`Resource`] containing a folder of arenas.
#[derive(Resource, Clone, Debug)]
pub struct ArenasHandle(Handle<AssetListing<Arena>>);

impl FromWorld for ArenasHandle {
    fn from_world(world: &mut World) -> Self {
        Self(world.resource::<AssetServer>().load("arenas.listing"))
    }
}

/// A [`SystemParam`] for accessing the loaded [`ArenaFolder`].
#[derive(SystemParam)]
pub struct Arenas<'w, 's> {
    handle: Res<'w, ArenasHandle>,
    listings: Res<'w, Assets<AssetListing<Arena>>>,
    arenas: Res<'w, Assets<Arena>>,
    asset_server: Res<'w, AssetServer>,
    commands: Commands<'w, 's>,
}

impl Arenas<'_, '_> {
    pub fn get_all(&self) -> Option<impl Iterator<Item = (AssetId<Arena>, &Arena)>> {
        let id = self.handle.0.id();

        if !self.asset_server.is_loaded_with_dependencies(id) {
            // Folder not loaded yet.
            return None;
        }
        let listing = self.listings.get(id).unwrap();
        let mut res = vec![];
        listing.visit_dependencies(&mut |id| {
            let id = id.typed::<Arena>();
            let arena = self.arenas.get(id).unwrap();
            res.push((id, arena));
        });
        Some(res.into_iter())
    }
}

#[derive(Debug, Clone, Default, Copy)]
pub struct ArenaPlugin;

impl Plugin for ArenaPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset_with_lifecycle::<Arena>()
            .init_asset_listing::<Arena>()
            .register_type::<Arena>()
            .init_asset_loader::<ArenaLoader>()
            .init_resource::<ArenasHandle>()
            .init_resource::<CachedArenaSpawnState>()
            .add_systems(PostStartup, spawn_default_arena);
    }
}

fn spawn_default_arena(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.run_system_cached_with(
        spawn_arena,
        asset_server.load::<Arena>(asset_path("ultimate/fru/p1")),
    );

    let waymarks = r#"{
  "Name":"TEA",
  "MapID":694,
  "A":{"X":100.0,"Y":0.0,"Z":88.0,"ID":0,"Active":true},
  "B":{"X":114.0,"Y":0.0,"Z":100.0,"ID":1,"Active":true},
  "C":{"X":100.0,"Y":0.0,"Z":116.0,"ID":2,"Active":true},
  "D":{"X":84.0,"Y":0.0,"Z":100.0,"ID":3,"Active":true},
  "One":{"X":92.2,"Y":0.0,"Z":107.8,"ID":4,"Active":true},
  "Two":{"X":100.0,"Y":0.0,"Z":107.8,"ID":5,"Active":true},
  "Three":{"X":107.8,"Y":0.0,"Z":107.8,"ID":6,"Active":true},
  "Four":{"X":107.8,"Y":0.0,"Z":100.0,"ID":7,"Active":true}
}"#;

    Waymark::spawn_from_preset(&mut commands, serde_json::de::from_str(waymarks).unwrap());
}

pub fn plugin() -> ArenaPlugin {
    ArenaPlugin
}
