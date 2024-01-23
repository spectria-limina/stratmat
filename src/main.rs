#![allow(dead_code)]

use bevy::prelude::*;
use bevy::winit::WinitSettings;
use bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_vector_shapes::prelude::*;

mod arena;
mod waymark;

use waymark::Waymark;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Stratmat".into(),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: false,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin)
        .add_plugins(Shape2dPlugin::default())
        .insert_resource(WinitSettings::desktop_app())
        .add_plugins(arena::plugin())
        .add_plugins(WorldInspectorPlugin::new())
        .add_systems(Startup, spawn_waymarks)
        .run();
}

const P9S_PRESET: &'static str = r#"{
  "Name":"P9S (JP)",
  "MapID":937,
  "A":{"X":100.0,"Y":0.0,"Z":86.0,"ID":0,"Active":true},
  "B":{"X":114.0,"Y":0.0,"Z":100.0,"ID":1,"Active":true},
  "C":{"X":100.0,"Y":0.0,"Z":114.0,"ID":2,"Active":true},
  "D":{"X":86.0,"Y":0.0,"Z":100.0,"ID":3,"Active":true},
  "One":{"X":109.899,"Y":0.0,"Z":90.1,"ID":4,"Active":true},
  "Two":{"X":109.899,"Y":0.0,"Z":109.899,"ID":5,"Active":true},
  "Three":{"X":90.1,"Y":0.0,"Z":109.899,"ID":6,"Active":true},
  "Four":{"X":90.1,"Y":0.0,"Z":90.1,"ID":7,"Active":true}
}"#;

pub fn spawn_waymarks(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    arena: Res<arena::Arena>,
) {
    let preset: waymark::Preset = serde_json::from_str(P9S_PRESET).unwrap();
    Waymark::spawn_from_preset(&mut commands, &*asset_server, arena.offset, &preset);
}
