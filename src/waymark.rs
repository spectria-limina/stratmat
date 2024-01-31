//! Waymark support.
//!
//! This module implements support for FFXIV waymarks.
//! Waymarks can be manually manipulated, as well as imported and exported using the format of the Waymark Preset plugin.

use bevy::ecs::system::{Command, CommandQueue, EntityCommand, EntityCommands, SystemId};
use bevy::prelude::*;
use bevy::window::RequestRedraw;
use bevy_egui::egui::{TextEdit, Ui};
use bevy_egui::{egui, EguiClipboard, EguiContexts};
use bevy_mod_picking::backend::{HitData, PointerHits};
use bevy_mod_picking::prelude::*;
use bevy_vector_shapes::prelude::*;
use enum_iterator::Sequence;
use int_enum::IntEnum;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

use crate::arena::Arena;
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

/// The size of waymark spawner, in pixels.
const WAYMARK_SPAWNER_SIZE: f32 = 40.0;
/// The alpha (out of 255) of an enabled waymark spawner widget.
const WAYMARK_SPAWNER_ALPHA: u8 = 230;
/// The alpha (out of 255) of a disabled waymark spawner widget.
const WAYMARK_SPAWNER_DISABLED_ALPHA: u8 = 25;

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
#[derive(
    Copy,
    Clone,
    Component,
    Serialize,
    Deserialize,
    Debug,
    Hash,
    PartialOrd,
    Ord,
    PartialEq,
    Eq,
    IntEnum,
    Sequence,
)]
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

    /// Retrieves a [Handle] to this image asset with the letter or number of the waymark.
    pub fn asset_handle(&self, asset_server: &AssetServer) -> Handle<Image> {
        asset_server.load(self.asset_path())
    }

    /// Produces the fill/stroke colour for this waymark.
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

    /// Produces a [PresetEntry] corresponding to this waymark,
    /// using the provided [Arena]'s center `offset` and the provided [Transform].
    pub fn to_entry(&self, transform: &Transform, offset: Vec2) -> PresetEntry {
        PresetEntry {
            x: offset.x + transform.translation.x,
            y: 0.0,
            // The entry's Z axis is our negative Y axis.
            z: offset.y - transform.translation.y,
            id: u8::from(*self),
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
        let arena = world.get_resource::<crate::arena::Arena>().unwrap();
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

#[derive(Debug, Resource)]
/// Resource storing the ID of [WaymarkWindow::export_to_clipboard].
pub struct ExportToClipboard(SystemId);

impl FromWorld for ExportToClipboard {
    fn from_world(world: &mut World) -> Self {
        Self(world.register_system(WaymarkWindow::export_to_clipboard))
    }
}

impl ExportToClipboard {
    fn id(&self) -> SystemId {
        self.0
    }
}

/// An entity that can be clicked & dragged to spawn a waymark.
///
/// Rendered using egui, not the normal logic.
#[derive(Debug, Clone, Component)]
pub struct Spawner {
    waymark: Waymark,
    texture_id: egui::TextureId,
}

impl Spawner {
    fn show(
        &self,
        id: Entity,
        ui: &mut Ui,
        waymark_q: &Query<&Waymark>,
        camera_q: &Query<(&Camera, &GlobalTransform)>,
        commands: &mut Commands,
        pointer_ev: &mut EventWriter<PointerHits>,
    ) {
        let enabled = !waymark_q.iter().contains(&self.waymark);
        let resp = ui.add(
            egui::Image::new((
                self.texture_id,
                egui::Vec2::new(WAYMARK_SPAWNER_SIZE, WAYMARK_SPAWNER_SIZE),
            ))
            .tint(egui::Color32::from_white_alpha(if enabled {
                WAYMARK_SPAWNER_ALPHA
            } else {
                WAYMARK_SPAWNER_DISABLED_ALPHA
            }))
            .sense(egui::Sense::drag()),
        );
        if resp.hovered() {
            let egui::Pos2 { x, y } = resp.hover_pos().unwrap();
            pointer_ev.send(PointerHits::new(
                PointerId::Mouse,
                vec![(id, HitData::new(id, 0.0, Some(Vec3::new(x, y, 0.0)), None))],
                // egui is at depth 1_000_000, we need to be in front of that.
                1_000_001.0,
            ))
        }
        if enabled && resp.drag_started_by(egui::PointerButton::Primary) {
            let (camera, camera_transform) = camera_q.single();
            let egui::Pos2 { x, y } = resp.rect.center();
            let vp_center = Vec2::new(x, y);
            let center = camera
                .viewport_to_world_2d(camera_transform, vp_center)
                .unwrap();

            // Rather than spawning a new waymark, turn this entity into the waymark and replace it with a new spawner.
            // This allows drag events to apply to the new waymark.
            commands.spawn(SpawnerBundle {
                name: Name::new(format!("Spawner for {}", self.waymark.name())),
                spawner: self.clone(),
                pickable: default(),
            });

            let mut entity_commands = commands.entity(id);
            entity_commands.remove::<SpawnerBundle>();

            self.waymark
                .spawn_inplace(entity_commands)
                .insert(Transform::from_translation((center, 0.0).into()));
        };
    }
}

/// Bundle of components for a [Spawner].
#[derive(Bundle)]
pub struct SpawnerBundle {
    name: Name,
    spawner: Spawner,
    pickable: PickableBundle,
}

impl SpawnerBundle {
    pub fn new(waymark: Waymark, asset_server: &AssetServer, contexts: &mut EguiContexts) -> Self {
        Self {
            name: Name::new(format!("Spawner for {}", waymark.name())),
            spawner: Spawner {
                waymark,
                texture_id: contexts.add_image(waymark.asset_handle(&asset_server)),
            },
            pickable: default(),
        }
    }
}

/// A window with controls to manipulate the waymarks.
#[derive(Debug, Default, Component)]
pub struct WaymarkWindow {
    preset_name: String,
}

impl WaymarkWindow {
    /// [System] that draws the waymark window and handles events.
    ///
    /// Will panic if there is more than one camera.
    pub fn draw(
        mut win_q: Query<&mut WaymarkWindow>,
        spawner_q: Query<(Entity, &Spawner)>,
        waymark_q: Query<&Waymark>,
        camera_q: Query<(&Camera, &GlobalTransform)>,
        mut commands: Commands,
        mut contexts: EguiContexts,
        clipboard: Res<EguiClipboard>,
        export_to_clipboard: Res<ExportToClipboard>,
        mut pointer_ev: EventWriter<PointerHits>,
    ) {
        let mut win = win_q.single_mut();

        let ewin = egui::Window::new("Waymarks").default_width(4.0 * WAYMARK_SPAWNER_SIZE);
        ewin.show(contexts.ctx_mut(), |ui| {
            ui.horizontal(|ui| {
                ui.label("Preset: ");
                ui.add(TextEdit::singleline(&mut win.preset_name).desired_width(100.0));
            });
            ui.horizontal(|ui| {
                if ui.button("Import").clicked() {
                    if let Some(contents) = clipboard.get_contents() {
                        match serde_json::from_str::<Preset>(&contents) {
                            Ok(preset) => {
                                win.preset_name = preset.name.clone();
                                commands.add(DespawnAll);
                                commands.add(SpawnFromPreset { preset });
                                log::info!(
                                    "Imported waymark preset '{}' from the clipboard",
                                    win.preset_name
                                );
                            }
                            Err(e) => {
                                log::info!("Unable to import waymark preset: {}", e);
                            }
                        }
                    } else {
                        log::info!("Unable to import waymark preset: clipboard is empty")
                    }
                }
                if ui.button("Export").clicked() {
                    commands.run_system(export_to_clipboard.id())
                }
                if ui.button("Clear").clicked() {
                    commands.despawn_all_waymarks()
                }
            });
            ui.separator();
            // TODO: Figure out how to make this not suck.
            let spawners: HashMap<_, _> = spawner_q
                .iter()
                .map(|(id, spawner @ &Spawner { waymark, .. })| (waymark, (id, spawner)))
                .collect();
            ui.horizontal(|ui| {
                let (id, spawner) = spawners[&Waymark::One];
                spawner.show(
                    id,
                    ui,
                    &waymark_q,
                    &camera_q,
                    &mut commands,
                    &mut pointer_ev,
                );
                let (id, spawner) = spawners[&Waymark::Two];
                spawner.show(
                    id,
                    ui,
                    &waymark_q,
                    &camera_q,
                    &mut commands,
                    &mut pointer_ev,
                );
                let (id, spawner) = spawners[&Waymark::Three];
                spawner.show(
                    id,
                    ui,
                    &waymark_q,
                    &camera_q,
                    &mut commands,
                    &mut pointer_ev,
                );
                let (id, spawner) = spawners[&Waymark::Four];
                spawner.show(
                    id,
                    ui,
                    &waymark_q,
                    &camera_q,
                    &mut commands,
                    &mut pointer_ev,
                );
            });
            ui.horizontal(|ui| {
                let (id, spawner) = spawners[&Waymark::A];
                spawner.show(
                    id,
                    ui,
                    &waymark_q,
                    &camera_q,
                    &mut commands,
                    &mut pointer_ev,
                );
                let (id, spawner) = spawners[&Waymark::B];
                spawner.show(
                    id,
                    ui,
                    &waymark_q,
                    &camera_q,
                    &mut commands,
                    &mut pointer_ev,
                );
                let (id, spawner) = spawners[&Waymark::C];
                spawner.show(
                    id,
                    ui,
                    &waymark_q,
                    &camera_q,
                    &mut commands,
                    &mut pointer_ev,
                );
                let (id, spawner) = spawners[&Waymark::D];
                spawner.show(
                    id,
                    ui,
                    &waymark_q,
                    &camera_q,
                    &mut commands,
                    &mut pointer_ev,
                );
            });
        });
    }

    /// [System] that exports the currently-spawned waymarks to the clipboard.
    pub fn export_to_clipboard(
        win_q: Query<&WaymarkWindow>,
        waymarks_q: Query<(&Waymark, &Transform)>,
        arena: Res<Arena>,
        mut clipboard: ResMut<EguiClipboard>,
    ) {
        let preset = Preset {
            name: win_q.single().preset_name.clone(),
            map_id: arena.map_id,
            waymarks: waymarks_q
                .iter()
                .map(|(&waymark, transform)| (waymark, waymark.to_entry(transform, arena.offset)))
                .collect(),
        };
        match serde_json::to_string(&preset) {
            Ok(json) => {
                clipboard.set_contents(&json);
                log::info!("Exported waymark preset '{}' to the clipboard", preset.name)
            }
            Err(e) => log::error!("Unable to serialize waymark preset for export: {e}"),
        }
    }
}

/// Plugin for the waymark window.
pub struct WaymarkPlugin;

impl Plugin for WaymarkPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, WaymarkWindow::draw)
            .add_systems(
                Startup,
                |mut commands: Commands,
                 asset_server: Res<AssetServer>,
                 mut contexts: EguiContexts| {
                    commands.spawn(WaymarkWindow::default());
                    for waymark in enum_iterator::all::<Waymark>() {
                        commands.spawn(SpawnerBundle::new(waymark, &asset_server, &mut contexts));
                    }
                },
            )
            .init_resource::<ExportToClipboard>();
    }
}

/// Produces a plugin.
pub fn plugin() -> WaymarkPlugin {
    WaymarkPlugin
}
