//! Waymark support.
//!
//! This module implements support for FFXIV waymarks.
//! Waymarks can be manually manipulated, as well as imported and exported using the format of the Waymark Preset plugin.

use bevy::ecs::system::{Command, CommandQueue, EntityCommand, EntityCommands};
use bevy::prelude::*;
use bevy::window::RequestRedraw;
use bevy_mod_picking::prelude::*;
use bevy_vector_shapes::prelude::*;
use enum_iterator::Sequence;
use int_enum::IntEnum;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

use crate::arena::ArenaData;
use crate::cursor::DraggableBundle;

pub mod window;

/// The diameter, in yalms, of a waymark.
const WAYMARK_SIZE: f32 = 2.4;
/// The scaling to apply to the waymark letter/number image.
const IMAGE_SCALE: f32 = 1.0;
/// The opacity of the fill of a waymark.
const FILL_OPACITY: f32 = 0.22;
/// The opacity of the outer line of a waymark.
const STROKE_OPACITY: f32 = 0.75;
/// The stroke width of the outer line of a waymark.
const STROKE_WIDTH: f32 = 0.05;

/// A waymark preset in the JSON format of the Waymark Preset plugin.
///
/// This type can be directly serialized from/to the Waymark Preset format.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Preset {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "MapID")]
    map_id: u32,

    #[serde(flatten)]
    waymarks: HashMap<Waymark, PresetEntry>,
}

/// A single waymark entry in the Waymark Preset format.
///
/// Coordinates are all in the FFXIV coordinate system, not the Stratmap coordinate system.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct PresetEntry {
    /// Corresponds to the X axis in Stratmap.
    #[serde(rename = "X")]
    x: f32,
    /// Would be the Z axis in Stratmap and therefore always ignored by us.
    #[serde(rename = "Y")]
    y: f32,
    /// Corresponds to the negative Y axis in Stratmap.
    #[serde(rename = "Z")]
    z: f32,
    /// Numeric ID of the waymark (redundant but important for the plugin).
    #[serde(rename = "ID")]
    id: u8,
    /// Whether the waymark is active. Stratmat simply discards inactive waymarks.
    #[serde(rename = "Active")]
    active: bool,
}

/// A placeable marker for players to reference movements during a fight.
#[repr(u8)]
#[derive(Copy, Clone, Debug, Hash, PartialOrd, Ord, PartialEq, Eq)]
#[derive(Component, Reflect, Serialize, Deserialize)]
#[derive(IntEnum, Sequence)]
pub enum Waymark {
    A = 0,
    B = 1,
    C = 2,
    D = 3,
    One = 4,
    Two = 5,
    Three = 6,
    Four = 7,
}

impl Waymark {
    /// Produces the asset path for the image with the letter or number of the waymark.
    pub fn asset_path(self) -> &'static str {
        match self {
            Waymark::One => "waymarks/way_1.png",
            Waymark::Two => "waymarks/way_2.png",
            Waymark::Three => "waymarks/way_3.png",
            Waymark::Four => "waymarks/way_4.png",
            Waymark::A => "waymarks/way_a.png",
            Waymark::B => "waymarks/way_b.png",
            Waymark::C => "waymarks/way_c.png",
            Waymark::D => "waymarks/way_d.png",
        }
    }

    /// Retrieves a [Handle] to this image asset with the letter or number of the waymark.
    pub fn asset_handle(self, asset_server: &AssetServer) -> Handle<Image> {
        asset_server.load(self.asset_path())
    }

    /// Produces the fill/stroke colour for this waymark.
    pub fn color(self) -> Color {
        match self {
            Waymark::One | Waymark::A => Color::RED,
            Waymark::Two | Waymark::B => Color::YELLOW,
            Waymark::Three | Waymark::C => Color::CYAN,
            Waymark::Four | Waymark::D => Color::FUCHSIA,
        }
    }

    /// Produces a name suitable for use as an entity label.
    fn name(self) -> &'static str {
        match self {
            Waymark::A => "Waymark A",
            Waymark::B => "Waymark B",
            Waymark::C => "Waymark C",
            Waymark::D => "Waymark D",
            Waymark::One => "Waymark 1",
            Waymark::Two => "Waymark 2",
            Waymark::Three => "Waymark 3",
            Waymark::Four => "Waymark 4",
        }
    }

    /// Produces a [PresetEntry] corresponding to this waymark,
    /// using the provided [Arena](crate::arena::Arena) center `offset` and the provided [Transform].
    pub fn to_entry(self, transform: &Transform, offset: Vec2) -> PresetEntry {
        PresetEntry {
            x: offset.x + transform.translation.x,
            y: 0.0,
            // The entry's Z axis is our negative Y axis.
            z: offset.y - transform.translation.y,
            id: u8::from(self),
            active: true,
        }
    }

    /// Spawns the entities for this waymark.
    ///
    /// The entities include the `Waymark` entity itself as well as the necessary sprite entities
    /// to render it correctly.
    ///
    /// The returned [WaymarkEntityCommands] can be used to configure the resulting waymark.
    pub fn spawn<'w, 's, 'a>(
        self,
        commands: &'a mut Commands<'w, 's>,
    ) -> WaymarkEntityCommands<'w, 's, 'a> {
        self.spawn_inplace(commands.spawn_empty())
    }

    /// Spawns the entities for this waymark with an existing entity ID.
    fn spawn_inplace<'w, 's, 'a>(
        self,
        mut commands: EntityCommands<'w, 's, 'a>,
    ) -> WaymarkEntityCommands<'w, 's, 'a> {
        log::debug!("spawning waymark {:?} inplace", self);
        commands.insert((
            self,
            Name::new(self.name()),
            PickableBundle::default(),
            DraggableBundle::default(),
            SpatialBundle::default(),
        ));
        commands.add(SpawnChildren);
        WaymarkEntityCommands(commands)
    }

    pub fn spawn_from_preset(commands: &mut Commands, preset: Preset) {
        commands.add(SpawnFromPreset { preset })
    }

    pub fn despawn_all(commands: &mut Commands) {
        commands.add(DespawnAll)
    }
}

/// Extension trait for [Commands] to add waymark command functionality.
trait CommandExts<'w, 's> {
    /// Spawn a given waymark as by sp
    fn spawn_waymark<'a>(&'a mut self, waymark: Waymark) -> WaymarkEntityCommands<'w, 's, 'a>;
    fn spawn_waymarks_from_preset(&mut self, preset: Preset);
    fn despawn_all_waymarks(&mut self);
}

impl<'w, 's> CommandExts<'w, 's> for Commands<'w, 's> {
    fn spawn_waymark<'a>(&'a mut self, waymark: Waymark) -> WaymarkEntityCommands<'w, 's, 'a> {
        waymark.spawn(self)
    }

    fn spawn_waymarks_from_preset(&mut self, preset: Preset) {
        Waymark::spawn_from_preset(self, preset)
    }

    fn despawn_all_waymarks(&mut self) {
        Waymark::despawn_all(self)
    }
}

/// [Command] to spawn all of the waymarks specified in the given `preset`.
pub struct SpawnFromPreset {
    pub preset: Preset,
}

impl Command for SpawnFromPreset {
    fn apply(self, world: &mut World) {
        let mut arena_q = world.query::<&ArenaData>();
        // TODO: This will crash if the arena isn't loaded yet.
        let arena = arena_q.get_single(world).unwrap();
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, world);
        for (waymark, entry) in &self.preset.waymarks {
            if entry.active {
                waymark.spawn(&mut commands).with_entry(entry, arena.offset);
            }
        }
        queue.apply(world);
    }
}

/// [Command] to despawn all active waymarks.
pub struct DespawnAll;

impl Command for DespawnAll {
    fn apply(self, world: &mut World) {
        let mut query = world.query_filtered::<Entity, &Waymark>();
        let entities = query.iter(world).collect_vec();
        for entity in entities {
            DespawnRecursive { entity }.apply(world);
        }
        world.send_event(RequestRedraw);
    }
}

/// [Command] to spawn child entities of a `parent` [Waymark] entity.
struct SpawnChildren;

impl SpawnChildren {
    /// Spawns a single shape, circle or rectangle, for this waymark according to the provided
    /// [ShapeConfig] and bearing the specified `name`.
    fn spawn_shape(
        waymark: Waymark,
        builder: &mut WorldChildBuilder,
        config: &ShapeConfig,
        name: &'static str,
    ) {
        match waymark {
            Waymark::One | Waymark::Two | Waymark::Three | Waymark::Four => builder.spawn((
                Name::new(name),
                ShapeBundle::rect(config, Vec2::new(WAYMARK_SIZE, WAYMARK_SIZE)),
            )),
            Waymark::A | Waymark::B | Waymark::C | Waymark::D => builder.spawn((
                Name::new(name),
                ShapeBundle::circle(config, WAYMARK_SIZE / 2.0),
            )),
        };
    }
}

impl EntityCommand for SpawnChildren {
    fn apply(self, id: Entity, world: &mut World) {
        let mut parent = world.entity_mut(id);
        let waymark = parent.get::<Waymark>().copied().unwrap();

        let asset_server = parent.world().get_resource::<AssetServer>().unwrap();
        let image = waymark.asset_handle(asset_server);

        parent.with_children(|parent| {
            parent.spawn((
                Name::new("Waymark Image"),
                SpriteBundle {
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(
                            WAYMARK_SIZE * IMAGE_SCALE,
                            WAYMARK_SIZE * IMAGE_SCALE,
                        )),
                        ..default()
                    },
                    texture: image,
                    ..default()
                },
            ));

            Self::spawn_shape(
                waymark,
                parent,
                &ShapeConfig {
                    color: waymark.color().with_a(STROKE_OPACITY),
                    thickness: STROKE_WIDTH,
                    hollow: true,
                    alpha_mode: AlphaMode::Blend,
                    transform: Transform::from_xyz(0.0, 0.0, -0.1),
                    ..ShapeConfig::default_2d()
                },
                "Waymark Stroke",
            );

            Self::spawn_shape(
                waymark,
                parent,
                &ShapeConfig {
                    color: waymark.color().with_a(FILL_OPACITY),
                    hollow: false,
                    alpha_mode: AlphaMode::Blend,
                    transform: Transform::from_xyz(0.0, 0.0, -0.2),
                    ..ShapeConfig::default_2d()
                },
                "Waymark Fill",
            );
        });
    }
}

/// A list of commands that will be run to modify a [Waymark] entity.
/// It supports all methods of a regular [EntityCommands].
/// All methods apply to the top-level [Waymark] entity, and not to sub-entities.
pub struct WaymarkEntityCommands<'w, 's, 'a>(pub EntityCommands<'w, 's, 'a>);

impl<'w, 's, 'a> WaymarkEntityCommands<'w, 's, 'a> {
    /// Apply the position from a [PresetEntry] to this waymark.
    ///
    /// Overwrites any previous [Transform].
    pub fn with_entry(&mut self, entry: &PresetEntry, offset: Vec2) -> &mut Self {
        self.0.insert(Transform::from_xyz(
            entry.x - offset.x,
            // The entry's Z axis is our negative Y axis.
            offset.y - entry.z,
            0.0,
        ));
        self
    }
}

impl<'w, 's, 'a> Deref for WaymarkEntityCommands<'w, 's, 'a> {
    type Target = EntityCommands<'w, 's, 'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'w, 's, 'a> DerefMut for WaymarkEntityCommands<'w, 's, 'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
