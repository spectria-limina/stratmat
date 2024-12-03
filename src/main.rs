#![allow(dead_code)]

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::winit::WinitSettings;
use bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_vector_shapes::prelude::*;
use clap::{ArgAction, Parser as _};
use waymark::WaymarkPlugin;

#[cfg(test)]
mod testing;

mod arena;
mod color;
mod cursor;
mod debug;
mod ecs;
mod hitbox;
mod spawner;
mod waymark;
mod widget;

/// Collision layers.
#[derive(PhysicsLayer, Default)]
pub enum Layer {
    #[default]
    None,
    /// Entities on this layer can have entities dragged onto them.
    ///
    /// See `mod` [`cursor`].
    DragSurface,
    /// Entities on this layer are
    /// Entities on this layer are currently being dragged.
    Dragged,
}

#[derive(clap::Parser, Resource, Clone, Debug)]
struct Args {
    /// Debug mode for the physics engine
    #[clap(long, env = "STRATMAT_DEBUG_PHYSICS", action = ArgAction::Set, default_value_t = false)]
    debug_physics: bool,
    #[clap(long, env = "STRATMAT_DEBUG_INSPECTOR", action = ArgAction::Set, default_value_t = cfg!(debug_assertions))]
    /// Enable the egui inspector
    debug_inspector: bool,
    #[clap(long, env = "STRATMAT_LOG_ASSET_EVENTS", action = ArgAction::Set, default_value_t = false)]
    /// Enable debug logging of asset events
    log_asset_events: bool,
    #[clap(long, env = "STRATMAT_LOG_COLLISION_EVENTS", action = ArgAction::Set, default_value_t = false)]
    /// Enable debug logging of collisions events
    log_collision_events: bool,
    #[cfg(target_arch = "wasm32")]
    #[clap(long, env = "STRATMAT_CANVAS_ID", actions = ArgAction::Set, default_value_t = None)]
    canvas_id: Option<String>,
}

fn main() -> eyre::Result<()> {
    let args = Args::parse();
    let mut app = App::new();

    #[cfg(not(target_arch = "wasm32"))]
    let primary_window = Window {
        title: "Stratmat".into(),
        ..default()
    };
    #[cfg(target_arch = "wasm32")]
    let primary_window = Window {
        title: "Stratmat".into(),
        canvas_id: args.canvas_id,
        fit_canvas_to_parent: true,
        prevent_default_event_handling: false,
        ..default()
    };

    app.insert_resource(args.clone())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(primary_window),
            ..default()
        }))
        .add_plugins(EguiPlugin)
        .add_plugins(Shape2dPlugin::default())
        .add_plugins(
            PhysicsPlugins::default(), /* FIXME: Re-disable once Jondolf/avian#571 is fixed.
                                          .disable::<IntegratorPlugin>()
                                          .disable::<SolverPlugin>()
                                          .disable::<SleepingPlugin>(),
                                       */
        )
        .add_plugins(arena::plugin())
        .add_plugins(color::plugin())
        .add_plugins(cursor::plugin())
        .add_plugins(WaymarkPlugin)
        .add_plugins(waymark::window::WaymarkWindowPlugin::default())
        .add_plugins(arena::menu::ArenaMenuPlugin)
        .insert_resource(WinitSettings::desktop_app())
        .add_systems(Startup, spawn_camera);
    /*
    .add_systems(Startup, |mut commands: Commands| {
        let mut entity = commands.spawn_empty();
        insert_hitbox(
            &mut entity,
            Hitbox::new(
                HitboxKind::Directional,
                bevy::color::palettes::css::SALMON.into(),
                10.0,
            ),
        );
    });
    */

    if args.debug_inspector {
        app.add_plugins(WorldInspectorPlugin::new());
    }
    if args.debug_physics {
        app.add_plugins(PhysicsDebugPlugin::default());
    }
    if args.log_asset_events {
        app.add_systems(PostUpdate, debug::log_asset_events::<arena::Arena>);
    }
    if args.log_collision_events {
        app.add_systems(PostUpdate, debug::log_events::<Collision>);
        app.add_systems(PostUpdate, debug::log_events::<CollisionStarted>);
        app.add_systems(PostUpdate, debug::log_events::<CollisionEnded>);
    }

    app.run();
    Ok(())
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((Camera2d, OrthographicProjection::default_2d()));
}
