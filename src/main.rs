#![allow(dead_code)]

use bevy::prelude::*;
use bevy::winit::WinitSettings;
use bevy_egui::EguiPlugin;
use bevy_mod_picking::debug::DebugPickingMode;
use bevy_mod_picking::DefaultPickingPlugins;
use bevy_vector_shapes::prelude::*;

#[cfg(debug_assertions)]
use bevy_inspector_egui::quick::WorldInspectorPlugin;

mod arena;
mod color;
mod cursor;
mod waymark;

#[cfg(test)]
mod testing;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
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
    .add_plugins(DefaultPickingPlugins)
    .insert_resource(WinitSettings::desktop_app())
    .add_plugins(arena::plugin())
    .add_plugins(color::plugin())
    .add_plugins(cursor::plugin())
    .add_plugins(waymark::window::WaymarkPlugin::default())
    .add_systems(Startup, configure_picker_debug);

    #[cfg(debug_assertions)]
    app.add_plugins(WorldInspectorPlugin::new());

    app.run();
}

fn configure_picker_debug(mut logging_next_state: ResMut<NextState<DebugPickingMode>>) {
    logging_next_state.set(if cfg!(debug_assertions) {
        DebugPickingMode::Normal
    } else {
        DebugPickingMode::Disabled
    })
}
