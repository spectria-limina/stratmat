//! Waymark tray and associated code.

use bevy::ecs::system::SystemId;
use bevy::prelude::*;
use bevy_egui::egui::{TextEdit, Ui};
use bevy_egui::{egui, EguiClipboard, EguiContexts};
use bevy_mod_picking::backend::{HitData, PointerHits};
use bevy_mod_picking::prelude::*;
use itertools::Itertools;
use std::collections::HashMap;

use super::{CommandExts, DespawnAll, Preset, SpawnFromPreset, Waymark};
use crate::arena::Arena;

/// The size of waymark spawner, in pixels.
const WAYMARK_SPAWNER_SIZE: f32 = 40.0;
/// The alpha (out of 255) of an enabled waymark spawner widget.
const WAYMARK_SPAWNER_ALPHA: u8 = 230;
/// The alpha (out of 255) of a disabled waymark spawner widget.
const WAYMARK_SPAWNER_DISABLED_ALPHA: u8 = 25;

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
#[derive(Debug, Clone, Component, Reflect)]
#[reflect(from_reflect = false)]
pub struct Spawner {
    waymark: Waymark,
    #[reflect(ignore)]
    texture_id: egui::TextureId,
}

/// Information required for communication between a [Spawner] and the UI function.
#[derive(Component, Clone, Debug, Default, Reflect)]
pub struct SpawnerUi {
    // Spawner -> UI
    pub enabled: bool,
    // UI -> Spawner
    pub center: Vec2,
    pub hover_pos: Option<Vec2>,
}

impl Spawner {
    /// System that extracts information from the entity that is needed for updating the UI.
    pub fn extract_ui(mut q: Query<(&Spawner, &mut SpawnerUi)>, waymark_q: Query<&Waymark>) {
        for (spawner, mut ui) in &mut q {
            ui.enabled = !waymark_q.iter().contains(&spawner.waymark);
        }
    }

    /// Handle a drag event, spawning a new [Waymark] in place of the current entity if
    /// the [Spawner] is enabled.
    ///
    /// Technically what it actually does is, to preserve continuity of the drag event,
    /// replaces this entity with the new waymark, and spawns a new [Spawner] in its place.
    ///
    /// Panics if there is more than one camera.
    pub fn drag_start(
        ev: Listener<Pointer<DragStart>>,
        spawner_q: Query<(&Spawner, &SpawnerUi)>,
        camera_q: Query<(&Camera, &GlobalTransform)>,
        mut commands: Commands,
    ) {
        let id = ev.listener();
        let Ok((spawner, ui)) = spawner_q.get(id) else {
            return;
        };
        if !ui.enabled {
            return;
        }
        commands.spawn(SpawnerBundle {
            name: Name::new(format!("Spawner for {}", spawner.waymark.name())),
            spawner: spawner.clone(),
            ui: ui.clone(),
            pickable: default(),
            drag_start: On::<Pointer<DragStart>>::run(Self::drag_start),
        });

        let mut entity_commands = commands.entity(id);
        entity_commands.remove::<SpawnerBundle>();

        let (camera, camera_transform) = camera_q.single();
        let translation = camera
            .viewport_to_world_2d(camera_transform, ui.center)
            .unwrap()
            .extend(0.0);

        spawner
            .waymark
            .spawn_inplace(entity_commands)
            .insert(Transform::from_translation(translation));
    }

    /// System that takes hover data from the UI and uses it to generate pointer events.
    pub fn generate_hits(
        q: Query<(Entity, &SpawnerUi), With<Spawner>>,
        mut pointer_ev: EventWriter<PointerHits>,
    ) {
        for (id, ui) in &q {
            if let Some(pos) = ui.hover_pos {
                pointer_ev.send(PointerHits::new(
                    PointerId::Mouse,
                    vec![(id, HitData::new(id, 0.0, Some(pos.extend(0.0)), None))],
                    // egui is at depth 1_000_000, we need to be in front of that.
                    1_000_001.0,
                ));
            }
        }
    }
}

impl SpawnerUi {
    /// Render this entity on the [Ui], updating the [SpawnerUi] component based on egui state.
    pub fn show(&mut self, ui: &mut egui::Ui, spawner: &Spawner) {
        let resp = ui.add(
            egui::Image::new((
                spawner.texture_id,
                egui::Vec2::new(WAYMARK_SPAWNER_SIZE, WAYMARK_SPAWNER_SIZE),
            ))
            .tint(egui::Color32::from_white_alpha(if self.enabled {
                WAYMARK_SPAWNER_ALPHA
            } else {
                WAYMARK_SPAWNER_DISABLED_ALPHA
            }))
            .sense(egui::Sense::drag()),
        );

        let egui::Pos2 { x, y } = resp.rect.center();
        self.center = Vec2::new(x, y);

        self.hover_pos = if resp.hovered() {
            let egui::Pos2 { x, y } = resp.hover_pos().unwrap();
            Some(Vec2::new(x, y))
        } else {
            None
        }
    }
}

/// Bundle of components for a [Spawner].
#[derive(Bundle)]
pub struct SpawnerBundle {
    pub name: Name,
    pub spawner: Spawner,
    pub ui: SpawnerUi,
    pub pickable: PickableBundle,
    pub drag_start: On<Pointer<DragStart>>,
}

impl SpawnerBundle {
    pub fn new(waymark: Waymark, asset_server: &AssetServer, contexts: &mut EguiContexts) -> Self {
        Self {
            name: Name::new(format!("Spawner for {}", waymark.name())),
            spawner: Spawner {
                waymark,
                texture_id: contexts.add_image(waymark.asset_handle(&asset_server)),
            },
            ui: default(),
            pickable: default(),
            drag_start: On::<Pointer<DragStart>>::run(Spawner::drag_start),
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
        mut spawner_q: Query<(&mut SpawnerUi, &Spawner)>,
        _waymark_q: Query<&Waymark>,
        _camera_q: Query<(&Camera, &GlobalTransform)>,
        mut commands: Commands,
        mut contexts: EguiContexts,
        clipboard: Res<EguiClipboard>,
        export_to_clipboard: Res<ExportToClipboard>,
        _pointer_ev: EventWriter<PointerHits>,
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

            let mut spawners: HashMap<_, _> = spawner_q
                .iter_mut()
                .map(|(ui, spawner)| (spawner.waymark, (ui, spawner)))
                .collect();
            let show = |&mut (ref mut spawner_ui, spawner): &mut (Mut<'_, SpawnerUi>, &Spawner),
                        ui: &mut Ui| { spawner_ui.show(ui, spawner) };

            ui.separator();
            ui.horizontal(|ui| {
                show(spawners.get_mut(&Waymark::One).unwrap(), ui);
                show(spawners.get_mut(&Waymark::Two).unwrap(), ui);
                show(spawners.get_mut(&Waymark::Three).unwrap(), ui);
                show(spawners.get_mut(&Waymark::Four).unwrap(), ui);
            });
            ui.horizontal(|ui| {
                show(spawners.get_mut(&Waymark::A).unwrap(), ui);
                show(spawners.get_mut(&Waymark::B).unwrap(), ui);
                show(spawners.get_mut(&Waymark::C).unwrap(), ui);
                show(spawners.get_mut(&Waymark::D).unwrap(), ui);
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

    /// Setup the window.
    pub fn setup(
        mut commands: Commands,
        asset_server: Res<AssetServer>,
        mut contexts: EguiContexts,
    ) {
        commands.spawn(WaymarkWindow::default());
        for waymark in enum_iterator::all::<Waymark>() {
            commands.spawn(SpawnerBundle::new(waymark, &asset_server, &mut contexts));
        }
    }
}

/// Plugin for the waymark window.
pub struct WaymarkPlugin;

impl Plugin for WaymarkPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, Spawner::extract_ui)
            .add_systems(Update, WaymarkWindow::draw)
            .add_systems(PostUpdate, Spawner::generate_hits)
            .add_systems(Startup, WaymarkWindow::setup)
            .init_resource::<ExportToClipboard>()
            .register_type::<Spawner>()
            .register_type::<SpawnerUi>();
    }
}

/// Produces a plugin.
pub fn plugin() -> WaymarkPlugin {
    WaymarkPlugin
}
