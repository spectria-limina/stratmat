#![allow(dead_code)]

use bevy::prelude::*;
use bevy::winit::WinitSettings;
use bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_mod_picking::debug::DebugPickingMode;
use bevy_mod_picking::DefaultPickingPlugins;
use bevy_vector_shapes::prelude::*;
use bevy_xpbd_2d::{
    plugins::{
        collision::narrow_phase::NarrowPhaseConfig, IntegratorPlugin, PhysicsDebugPlugin,
        PhysicsPlugins, SleepingPlugin, SolverPlugin,
    },
    prelude::PhysicsLayer,
};
use clap::{ArgAction, Parser as _};
use waymark::WaymarkPlugin;

#[cfg(test)]
mod testing;

mod arena;
mod color;
mod cursor;
mod ecs;
mod spawner;
mod waymark;
mod widget;

/// Collision layers.
#[derive(PhysicsLayer)]
pub enum Layer {
    /// Entities on this layer can have entities dragged onto them.
    ///
    /// See `mod` [`cursor`].
    DragSurface,
    /// Entities on this layer are currently being dragged.
    Dragged,
}

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
    #[clap(long, env = "STRATMAT_DEBUG_PICKING", default_value = "disabled")]
    /// Debug mode for bevy_mod_picking
    debug_picking: ArgDebugPickingMode,
    /// Debug mode for the physics engine
    #[clap(long, env = "STRATMAT_DEBUG_PHYSICS", action = ArgAction::Set, default_value_t = false)]
    debug_physics: bool,
    #[clap(long, env = "STRATMAT_DEBUG_PICKING", action = ArgAction::Set, default_value_t = cfg!(debug_assertions))]
    /// Control if the egui inspector is enabled or not
    debug_inspector: bool,
}

fn main() -> eyre::Result<()> {
    let args = Args::parse();

    let mut app = App::new();
    app.insert_resource(args.clone())
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
        .add_plugins(DefaultPickingPlugins)
        .add_plugins(
            PhysicsPlugins::default()
                .build()
                .disable::<IntegratorPlugin>()
                .disable::<SolverPlugin>()
                .disable::<SleepingPlugin>(),
        )
        // FIXME: Remove this once Jondolf/bevy_xpbd#224 is fixed.
        .insert_resource(NarrowPhaseConfig {
            prediction_distance: 0.0,
        })
        .add_plugins(arena::plugin())
        .add_plugins(color::plugin())
        .add_plugins(cursor::plugin())
        .add_plugins(WaymarkPlugin)
        .add_plugins(waymark::window::WaymarkWindowPlugin::default())
        .add_plugins(arena::menu::ArenaMenuPlugin)
        .insert_resource(WinitSettings::desktop_app())
        .add_systems(Startup, spawn_camera)
        .add_systems(Startup, configure_picker_debug);

    if args.debug_inspector {
        app.add_plugins(WorldInspectorPlugin::new());
    }
    if args.debug_physics {
        app.add_plugins(PhysicsDebugPlugin::default());
    }

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
