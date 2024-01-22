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

pub fn spawn_waymarks(mut commands: Commands, asset_server: Res<AssetServer>) {
    Waymark::A.spawn(&mut commands, &*asset_server);
    Waymark::Two
        .spawn(&mut commands, &*&asset_server)
        .with_transform(Transform::from_xyz(8.0, 0.0, 0.0));
}
