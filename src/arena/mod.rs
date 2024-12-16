use std::{
    io,
    path::{Path, PathBuf},
};

use avian2d::prelude::*;
use bevy::{
    asset::{AssetLoader, ParseAssetPathError},
    prelude::*,
};
use component::{ArenaWebComponents, ARENA_COMPONENT_TAG};
use custom_elements::CustomElement;
use itertools::Itertools;
use serde::Deserialize;
use thiserror::Error;

use crate::{
    asset::{AssetHookExt, AssetHookTarget, AssetListing, LifecycleExts, ListingExt},
    image::DrawImage,
    shape::{ColliderFromShape, Shape},
    waymark::{Preset, Waymark},
    Layer,
};

#[cfg(feature = "egui")]
mod menu_egui;
pub mod menu {
    #[cfg(feature = "egui")]
    pub use super::menu_egui::*;
}

#[cfg(feature = "dom")]
mod component_dom;
pub mod component {
    #[cfg(feature = "dom")]
    pub use super::component_dom::*;
}

/// The file extension of `Arena` files.
const EXTENSION: &str = "arena.ron";
/// The path, relative to the assets directory, to the directory where `Arena` files are stored.
const DIR: &str = "arenas";

const ARENA_LISTING_PATH: &str = "arenas/.listing";

/// Get the asset path for an arena, given its path minus the
/// constant directory and extension parts.
pub fn asset_path(arena: impl AsRef<Path>) -> PathBuf {
    let mut path = PathBuf::new();
    path.push(DIR);
    path.push(arena);
    path.set_extension(EXTENSION);
    path
}

/// An [`Arena`] is the backdrop to a fight, and includes everything needed to stage and set up a fight,
/// such as the arena's background image, dimensions, and other metadata.
#[derive(Asset, Reflect, Clone, Debug, Deserialize)]
pub struct ArenaMeta {
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
    pub shape: Shape,
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
    type Asset = ArenaMeta;
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
        let mut data: ArenaMeta = ron::de::from_bytes(&buf)?;
        data.background_path = load_context
            .asset_path()
            .resolve(&data.background_path)?
            .to_string();
        Ok(data)
    }

    fn extensions(&self) -> &[&str] { &[EXTENSION] }
}

/// Component for the current arena.
///
/// Currently only one is allowed at a time.
///
/// TODO: Make more than one allowed at a time?
#[derive(Deref, Component, Reflect, Clone, Debug)]
#[require(Transform)]
#[cfg_attr(feature = "egui", require(Sprite, Visibility))]
pub struct Arena(pub ArenaMeta);

/// How big the viewport should be relative to the size of the arena.
const ARENA_VIEWPORT_SCALE: f32 = 1.1;

/// Z-coordinate of the arena background.
const ARENA_BACKGROUND_Z: f32 = 0.0;

/// This resource represents the global coordinate offset for
/// game coordinates. It is updated whenever an arena is spawned.
///
/// It does not implement Default because (0,0) is probably the
/// wrong offset.
#[derive(Deref, Resource, Copy, Clone, Debug)]
pub struct GameCoordOffset(pub Vec2);

/// Event that is triggered when an arena is loaded, tageting the new arena.
#[derive(Copy, Clone, Debug, Event, Reflect)]
pub struct ArenaLoaded;

/// Spawn an arena
///
/// This includes resetting the camera and updating the [`GameCoordOffset`].
fn spawn_arena(
    In(arena): In<ArenaMeta>,
    #[cfg(feature = "egui")] mut camera_q: Query<
        &'static mut OrthographicProjection,
        With<Camera2d>,
    >,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    info!("Spawning new arena: {}", arena.name);
    // FIXME: Single-camera assumption.
    #[cfg(feature = "egui")]
    {
        use bevy::render::camera::ScalingMode;
        camera_q.single_mut().scaling_mode = ScalingMode::AutoMin {
            min_width: arena.size.x * ARENA_VIEWPORT_SCALE,
            min_height: arena.size.y * ARENA_VIEWPORT_SCALE,
        };
    }
    let mut entity = commands.spawn((
        Arena(arena.clone()),
        Name::new("Arena Background"),
        DrawImage::new(arena.background_path.into(), arena.size),
        Transform::from_xyz(0.0, 0.0, ARENA_BACKGROUND_Z),
        arena.shape,
        ColliderFromShape,
        CollisionLayers::new([Layer::DragSurface], [Layer::Dragged]),
        PickingBehavior::IGNORE,
    ));
    #[cfg(feature = "egui")]
    entity.insert(Sprite::default());
    let id = entity.id();

    commands.insert_resource(GameCoordOffset(arena.offset));
    commands.trigger_targets(ArenaLoaded, id);
}

/// Despawn all arenas.
pub fn despawn_all_arenas(world: &mut World) {
    let mut q = world.query_filtered::<Entity, With<Arena>>();
    for id in q.iter(world).collect_vec() {
        world.despawn(id);
    }
}

type ArenaListing = AssetListing<ArenaMeta>;

#[derive(Debug, Clone, Default, Copy)]
pub struct ArenaPlugin;

impl Plugin for ArenaPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset_with_lifecycle::<ArenaMeta>()
            .init_asset_listing::<ArenaMeta>()
            .register_type::<ArenaMeta>()
            .init_asset_loader::<ArenaLoader>()
            .load_global_asset::<ArenaListing>(ARENA_LISTING_PATH)
            .add_systems(Startup, spawn_default_arena);

        #[cfg(feature = "dom")]
        ArenaWebComponents::define(ARENA_COMPONENT_TAG);
        #[cfg(feature = "dom")]
        app.init_non_send_resource::<ArenaWebComponents>()
            .add_systems(First, ArenaWebComponents::sync_web_components)
            .add_systems(Last, Arena::display_web);
    }
}

fn spawn_default_arena(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle = asset_server.load::<ArenaMeta>(asset_path("ultimate/fru/p1"));
    commands.on_asset_loaded(
        handle.clone(),
        |arena: AssetHookTarget<ArenaMeta>, mut commands: Commands| {
            commands.run_system_cached_with(spawn_arena, arena.clone());
        },
    );

    commands.add_observer(|ev: Trigger<ArenaLoaded>, mut commands: Commands| {
        commands.entity(ev.observer()).despawn();
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
        let preset: Preset = serde_json::de::from_str(waymarks).unwrap();
        Waymark::spawn_from_preset(&mut commands, preset, ev.entity());
    });
}

pub fn plugin() -> ArenaPlugin { ArenaPlugin }
