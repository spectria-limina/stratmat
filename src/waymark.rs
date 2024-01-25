//! Waymark support.
//!
//! This module implements support for FFXIV waymarks.
//! Waymarks can be manually manipulated, as well as imported and exported using the format of the Waymark Preset plugin.

use bevy::ecs::system::EntityCommands;
use bevy::prelude::*;
use bevy_mod_picking::prelude::*;
use bevy_vector_shapes::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::cursor::DraggableBundle;

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
#[repr(C)]
#[derive(
    Copy, Clone, Component, Serialize, Deserialize, Debug, Hash, PartialOrd, Ord, PartialEq, Eq,
)]
pub enum Waymark {
    A,
    B,
    C,
    D,
    One,
    Two,
    Three,
    Four,
}

impl Waymark {
    /// Produces the asset path for the image with the letter or number of the waymark.
    pub fn asset_path(&self) -> &'static str {
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

    pub fn color(&self) -> Color {
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

    /// Spawns a single shape, circle or rectangle, for this waymark according to the provided
    /// [[ShapeConfig]] and bearing the specified `name`.
    fn spawn_shape(&self, builder: &mut ChildBuilder, config: &ShapeConfig, name: &'static str) {
        match self {
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

    /// Spawns the entities for this waymark.
    ///
    /// The entities include the `Waymark` entity itself as well as the necessary sprite entities
    /// to render it correctly.
    ///
    /// The returned [[WaymarkEntityCommands]] can be used to configure the resulting waymark.
    pub fn spawn<'w, 's, 'a>(
        self,
        commands: &'a mut Commands<'w, 's>,
        asset_server: &AssetServer,
    ) -> WaymarkEntityCommands<'w, 's, 'a> {
        let mut entity_commands = commands.spawn((
            self,
            Name::new(self.name()),
            PickableBundle::default(),
            DraggableBundle::default(),
            SpriteBundle {
                sprite: Sprite {
                    custom_size: Some(Vec2::new(
                        WAYMARK_SIZE * IMAGE_SCALE,
                        WAYMARK_SIZE * IMAGE_SCALE,
                    )),
                    ..default()
                },
                texture: asset_server.load(self.asset_path()),
                ..default()
            },
        ));

        entity_commands.with_children(|parent| {
            self.spawn_shape(
                parent,
                &ShapeConfig {
                    color: self.color().with_a(STROKE_OPACITY),
                    thickness: STROKE_WIDTH,
                    hollow: true,
                    alpha_mode: AlphaMode::Blend,
                    transform: Transform::from_xyz(0.0, 0.0, -0.1),
                    ..ShapeConfig::default_2d()
                },
                "Waymark Stroke",
            );

            self.spawn_shape(
                parent,
                &ShapeConfig {
                    color: self.color().with_a(FILL_OPACITY),
                    hollow: false,
                    alpha_mode: AlphaMode::Blend,
                    transform: Transform::from_xyz(0.0, 0.0, -0.2),
                    ..ShapeConfig::default_2d()
                },
                "Waymark Fill",
            );
        });
        WaymarkEntityCommands { entity_commands }
    }

    /// Spawns all of the waymarks specified in the given `preset`.
    ///
    /// An `offset` must be provided representing the X and Y (or, rather, Z) coordinates
    /// of the center of the boss arena, in yalms. This is frequently taken from the `offset`
    /// field of an [[Arena]]. The offset is required because Stratmap treats the center of
    /// the boss arena as the origin, but waymark presets use the in-game coordinates.
    pub fn spawn_from_preset(
        commands: &mut Commands,
        asset_server: &AssetServer,
        offset: Vec2,
        preset: &Preset,
    ) {
        for (waymark, entry) in &preset.waymarks {
            if entry.active {
                waymark
                    .spawn(commands, asset_server)
                    .with_entry(entry, offset);
            }
        }
    }
}

/// A list of commands that will be run to modify a [[Waymark]] entity.
pub struct WaymarkEntityCommands<'w, 's, 'a> {
    /// These entity commands correspond to the top-level [[Waymark]] entity only.
    pub entity_commands: EntityCommands<'w, 's, 'a>,
}

impl<'w, 's, 'a> WaymarkEntityCommands<'w, 's, 'a> {
    /// Apply the position from a [[PresetEntry]] to this waymark.
    ///
    /// Overwrites any previous [[Transform]].
    pub fn with_entry(&mut self, entry: &PresetEntry, offset: Vec2) -> &mut Self {
        self.entity_commands.insert(Transform::from_xyz(
            entry.x - offset.x,
            // The entry's Z axis is our negative Y axis.
            offset.y - entry.z,
            0.0,
        ));
        self
    }

    /// Apply the provided transform to this waymark.
    ///
    /// Overwrites any previous [[Transform]].
    pub fn with_transform(&mut self, transform: Transform) -> &mut Self {
        self.entity_commands.insert(transform);
        self
    }
}
