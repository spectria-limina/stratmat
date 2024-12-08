use std::{
    io,
    path::{Path, PathBuf},
};

use avian2d::prelude::*;
use bevy::{
    asset::{AssetLoader, ParseAssetPathError},
    prelude::*,
    render::camera::ScalingMode,
};
use bevy_inspector_egui::InspectorOptions;
use itertools::Itertools;
use serde::Deserialize;
use thiserror::Error;

use crate::{
    asset::{
        lifecycle::{AssetHookExt, AssetHookTarget, LifecycleExts},
        listing::{AssetListing, ListingExt},
    },
    waymark::{Preset, Waymark},
    Layer, PrimaryCamera,
};

pub mod menu;

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

    fn extensions(&self) -> &[&str] {
        &[EXTENSION]
    }
}

/// Component for the current arena.
///
/// Currently only one is allowed at a time.
///
/// TODO: Make more than one allowed at a time?
#[derive(Deref, Component, Reflect, Clone, Debug)]
#[require(Sprite, Transform, Visibility)]
pub struct Arena(pub ArenaMeta);

/// How big the viewport should be relative to the size of the arena.
const ARENA_VIEWPORT_SCALE: f32 = 1.1;

/// Z-coordinate of the arena background.
///
/// Our default viewport is (-1000.0, 1000.0), so make sure we are ever so slightly inside that.
const ARENA_BACKGROUND_Z: f32 = -999.0;

/// This resource represents the global coordinate offset for
/// game coordinates. It is updated whenever an arena is spawned.
///
/// It does not implement Default because (0,0) is probably the
/// wrong offset.
#[derive(Resource, Copy, Clone, Debug)]
pub struct GameCoordOffset(pub Vec2);

/// Spawn an arena
///
/// This includes resetting the camera and updating the [`GameCoordOffset`].
fn spawn_arena(
    In(arena): In<ArenaMeta>,
    mut camera_q: Query<&'static mut OrthographicProjection, With<PrimaryCamera>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    info!("Spawning new arena: {}", arena.name);
    camera_q.single_mut().scaling_mode = ScalingMode::AutoMin {
        min_width: arena.size.x * ARENA_VIEWPORT_SCALE,
        min_height: arena.size.y * ARENA_VIEWPORT_SCALE,
    };
    let background = asset_server.load(&arena.background_path);
    commands.spawn((
        Arena(arena.clone()),
        Name::new("Arena Background"),
        Sprite {
            image: background,
            custom_size: Some(arena.size),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, ARENA_BACKGROUND_Z),
        Collider::from(arena.shape),
        CollisionLayers::new([Layer::DragSurface], [Layer::Dragged]),
        PickingBehavior::IGNORE,
    ));
    commands.insert_resource(GameCoordOffset(arena.offset));
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
    commands.on_asset_loaded_with(
        handle,
        |In(preset), mut commands: Commands| Waymark::spawn_from_preset(&mut commands, preset),
        preset,
    );
}

pub fn plugin() -> ArenaPlugin {
    ArenaPlugin
}
