#![allow(dead_code)]

use bevy::prelude::*;
use bevy::winit::WinitSettings;
use bevy_egui::EguiPlugin;
use bevy_mod_picking::debug::DebugPickingMode;
use bevy_mod_picking::DefaultPickingPlugins;
use bevy_vector_shapes::prelude::*;
use clap::Parser as _;

#[cfg(debug_assertions)]
use bevy_inspector_egui::quick::WorldInspectorPlugin;

#[cfg(test)]
mod testing;

mod arena;
mod color;
mod cursor;
mod systems;
mod waymark;

/// Reimplementation of [DebugPickingMode] for use as a program argument
#[derive(clap::ValueEnum)]
#[derive(Default, Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
enum ArgDebugPickingMode {
    /// Debugging disabled
    #[default]
    Disabled,
    /// Pointer debugging enabled
    Normal,
    /// Pointer debugging and logspam enabled
    Noisy,
}

impl From<ArgDebugPickingMode> for DebugPickingMode {
    fn from(value: ArgDebugPickingMode) -> Self {
        match value {
            ArgDebugPickingMode::Disabled => DebugPickingMode::Disabled,
            ArgDebugPickingMode::Normal => DebugPickingMode::Normal,
            ArgDebugPickingMode::Noisy => DebugPickingMode::Noisy,
        }
    }
}

#[derive(clap::Parser, Resource, Clone, Debug)]
struct Args {
    #[clap(long, env = "STRATMAT_DEBUG_PICKING")]
    #[cfg_attr(debug_assertions, clap(default_value = "normal"))]
    #[cfg_attr(not(debug_assertions), clap(default_value = "disabled"))]
    /// Debug mode for bevy_mod_picking
    debug_picking: ArgDebugPickingMode,
}

fn main() -> eyre::Result<()> {
    let args = Args::parse();

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
    .add_plugins(arena::plugin())
    .add_plugins(color::plugin())
    .add_plugins(cursor::plugin())
    .add_plugins(waymark::window::WaymarkPlugin::default())
    .insert_resource(WinitSettings::desktop_app())
    .insert_resource(args)
    .add_systems(Startup, spawn_camera)
    .add_systems(Startup, configure_picker_debug);

    #[cfg(debug_assertions)]
    app.add_plugins(WorldInspectorPlugin::new());

    //bevy_mod_debugdump::print_schedule_graph(&mut app, PreUpdate);

    app.run();
    Ok(())
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2dBundle {
        projection: OrthographicProjection {
            near: -1000.0,
            far: 1000.0,
            ..default()
        },
        ..default()
    });
}

fn configure_picker_debug(
    args: Res<Args>,
    mut logging_next_state: ResMut<NextState<DebugPickingMode>>,
) {
    logging_next_state.set(args.debug_picking.into());
}
