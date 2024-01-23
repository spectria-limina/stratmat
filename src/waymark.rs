use bevy::ecs::system::EntityCommands;
use bevy::prelude::*;
use bevy_vector_shapes::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const WAYMARK_SIZE: f32 = 2.4;
const IMAGE_SCALE: f32 = 1.0;
const FILL_OPACITY: f32 = 0.22;
const STROKE_OPACITY: f32 = 0.75;
const STROKE_WIDTH: f32 = 0.05;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Preset {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "MapID")]
    map_id: u32,

    #[serde(flatten)]
    waymarks: HashMap<Waymark, PresetEntry>,
}

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
    /// Numeric ID of the waymark (redundant but important).
    #[serde(rename = "ID")]
    id: u8,
    /// We just discard inactive waymarks.
    #[serde(rename = "Active")]
    active: bool,
}

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

    fn spawn_shape(&self, builder: &mut ChildBuilder, config: &ShapeConfig, name: &'static str) {
        match self {
            Waymark::One | Waymark::Two | Waymark::Three | Waymark::Four => builder.spawn((
                ShapeBundle::rect(config, Vec2::new(WAYMARK_SIZE, WAYMARK_SIZE)),
                Name::new(name),
            )),
            Waymark::A | Waymark::B | Waymark::C | Waymark::D => builder.spawn((
                ShapeBundle::circle(config, WAYMARK_SIZE / 2.0),
                Name::new(name),
            )),
        };
    }

    pub fn spawn<'w, 's, 'a>(
        self,
        commands: &'a mut Commands<'w, 's>,
        asset_server: &AssetServer,
    ) -> WaymarkEntityCommands<'w, 's, 'a> {
        let mut entity_commands = commands.spawn((
            WaymarkBundle {
                waymark: self,
                spatial: default(),
            },
            Name::new(self.name()),
        ));
        entity_commands.with_children(|parent| {
            parent.spawn((
                SpriteBundle {
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(
                            WAYMARK_SIZE * IMAGE_SCALE,
                            WAYMARK_SIZE * IMAGE_SCALE,
                        )),
                        ..default()
                    },
                    transform: Transform::from_xyz(0.0, 0.0, 2.0),
                    texture: asset_server.load(self.asset_path()),
                    ..default()
                },
                Name::new("Waymark Image"),
            ));

            self.spawn_shape(
                parent,
                &ShapeConfig {
                    color: self.color().with_a(STROKE_OPACITY),
                    thickness: STROKE_WIDTH,
                    hollow: true,
                    alpha_mode: AlphaMode::Blend,
                    transform: Transform::from_xyz(0.0, 0.0, 1.0),
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
                    transform: Transform::from_xyz(0.0, 0.0, 1.0),
                    ..ShapeConfig::default_2d()
                },
                "Waymark Fill",
            );
        });
        WaymarkEntityCommands { entity_commands }
    }

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

pub struct WaymarkEntityCommands<'w, 's, 'a> {
    entity_commands: EntityCommands<'w, 's, 'a>,
}

impl<'w, 's, 'a> WaymarkEntityCommands<'w, 's, 'a> {
    pub fn with_entry(&mut self, entry: &PresetEntry, offset: Vec2) -> &mut Self {
        self.entity_commands.insert(Transform::from_xyz(
            entry.x - offset.x,
            // The use of Z here is intentional; see docs for PresetEntry.
            offset.y - entry.z,
            0.0,
        ));
        self
    }

    pub fn with_transform(&mut self, transform: Transform) -> &mut Self {
        self.entity_commands.insert(transform);
        self
    }
}

#[derive(Bundle)]
struct WaymarkBundle {
    pub waymark: Waymark,
    pub spatial: SpatialBundle,
}
