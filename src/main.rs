#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(dead_code)]
#![allow(unexpected_cfgs)]
// I'll clean em up later
#![allow(unused)]
// `rustdoc_internals` is needed for `#[doc(fake_variadics)]`
#![allow(internal_features)]
#![cfg_attr(any(docsrs, docsrs_dep), feature(rustdoc_internals))]

#[cfg(all(feature = "winit", feature = "dom"))]
compile_error!("Features 'winit' and 'dom' are incompatible.");

use std::path::{Path, PathBuf};

use avian2d::prelude::*;
#[cfg(feature = "egui")]
use bevy::winit::WinitSettings;
use bevy::{input::InputPlugin, log::LogPlugin, prelude::*};
#[cfg(feature = "egui")]
use bevy_egui::EguiPlugin;
#[cfg(feature = "egui")]
use bevy_inspector_egui::quick::WorldInspectorPlugin;
#[cfg(feature = "egui")]
use bevy_vector_shapes::Shape2dPlugin;
use clap::{ArgAction, Parser as _};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

mod arena;
mod asset;
mod color;
mod debug;
mod drag;
mod ecs;
mod hitbox;
mod image;
mod player;
mod shape;
mod spawner;
#[cfg(test)]
mod testing;
mod waymark;
mod widget;

/// Collision layers.
// avian's derive macro causes this warning on nightly
#[derive(PhysicsLayer, Default)]
pub enum Layer {
    #[default]
    None,
    /// Entities on this layer can have entities dragged onto them.
    ///
    /// See `mod` [`cursor`].
    DragSurface,
    /// Entities on this layer are currently being dragged.
    Dragged,
}

#[derive(clap::Parser, Resource, Clone, Debug)]
struct Args {
    /// Debug mode for the physics engine
    #[cfg(feature = "egui")]
    #[clap(long, env = "STRATMAT_DEBUG_PHYSICS", action = ArgAction::Set, default_value_t = false)]
    debug_physics: bool,
    #[cfg(feature = "egui")]
    #[clap(long, env = "STRATMAT_DEBUG_INSPECTOR", action = ArgAction::Set, default_value_t = cfg!(debug_assertions))]
    /// Enable the egui inspector
    debug_inspector: bool,
    #[clap(long, env = "STRATMAT_LOG_ASSET_EVENTS", action = ArgAction::Set, default_value_t = false)]
    /// Enable debug logging of asset events
    log_asset_events: bool,
    #[clap(long, env = "STRATMAT_LOG_COLLISION_EVENTS", action = ArgAction::Set, default_value_t = false)]
    /// Enable debug logging of collisions events
    log_collision_events: bool,
    #[clap(long, short)]
    asset_root: Option<PathBuf>,
    #[clap(long, short)]
    log_filter: Option<String>,
}

fn start(args: Args, #[cfg(feature = "egui")] primary_window: Window) -> eyre::Result<()> {
    let mut app = App::new();

    if let Some(ref path) = args.asset_root {
        set_root_asset_path(&mut app, path);
    }

    let mut log_plugin = LogPlugin::default();
    if let Some(ref filter) = args.log_filter {
        log_plugin.filter = filter.clone();
    }

    let mut default_plugins = DefaultPlugins.set(log_plugin).set(AssetPlugin {
        meta_check: bevy::asset::AssetMetaCheck::Never,
        ..default()
    });
    #[cfg(feature = "egui")]
    {
        default_plugins = default_plugins.set(WindowPlugin {
            primary_window: Some(primary_window),
            ..default()
        });
    }
    #[cfg(feature = "dom")]
    {
        default_plugins = default_plugins.disable::<InputPlugin>();
    }

    app.insert_resource(args.clone())
        .add_plugins(default_plugins)
        .add_plugins(
            PhysicsPlugins::default(), /* FIXME: Re-disable once Jondolf/avian#571 is fixed.
                                          .disable::<IntegratorPlugin>()
                                          .disable::<SolverPlugin>()
                                          .disable::<SleepingPlugin>(),
                                       */
        )
        .add_plugins(asset::plugin())
        .add_plugins(arena::plugin())
        .add_plugins(color::plugin())
        .add_plugins(drag::plugin())
        .add_plugins(ecs::plugin())
        .add_plugins(player::plugin())
        .add_plugins(shape::plugin())
        .add_plugins(waymark::plugin());

    #[cfg(feature = "egui")]
    app.add_plugins(EguiPlugin)
        .insert_resource(WinitSettings::desktop_app())
        .add_plugins(arena::menu::plugin())
        .add_plugins(Shape2dPlugin::default())
        .add_plugins(player::window::plugin())
        .add_plugins(waymark::window::plugin())
        .add_systems(Startup, spawn_camera);

    #[cfg(feature = "egui")]
    if args.debug_inspector {
        app.add_plugins(WorldInspectorPlugin::new());
    }
    #[cfg(feature = "egui")]
    if args.debug_physics {
        app.add_plugins(PhysicsDebugPlugin::default());
    }
    if args.log_asset_events {
        app.add_systems(PostUpdate, debug::log_asset_events::<arena::ArenaMeta>);
    }
    if args.log_collision_events {
        app.add_systems(PostUpdate, debug::log_events::<Collision>);
        app.add_systems(PostUpdate, debug::log_events::<CollisionStarted>);
        app.add_systems(PostUpdate, debug::log_events::<CollisionEnded>);
    }

    app.run();
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn set_root_asset_path(app: &mut App, path: &Path) {
    use bevy::asset::io::{file::FileAssetReader, AssetSource, AssetSourceId};
    let path = path.to_owned();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSource::build().with_reader(move || Box::new(FileAssetReader::new(path.clone()))),
    );
}

#[cfg(target_arch = "wasm32")]
fn set_root_asset_path(app: &mut App, path: &Path) {
    use bevy::asset::io::{wasm::HttpWasmAssetReader, AssetSource, AssetSourceId};
    let path = path.to_owned();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSource::build().with_reader(move || Box::new(HttpWasmAssetReader::new(path.clone()))),
    );
}

#[cfg(feature = "egui")]
fn spawn_camera(mut commands: Commands) {
    commands.spawn((Camera2d, OrthographicProjection::default_2d()));
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eyre::Result<()> {
    let primary_window = Window {
        title: "Stratmat".into(),
        ..default()
    };
    start(Args::parse(), primary_window)
}

// on the web. So work around that a bit.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(main)]
fn main() -> Result<(), JsValue> {
    use convert_case::{Case, Casing};
    use web_sys::console;
    console::log_1(&"stratmat init: initializing...".into());

    let selector = option_env!("STRATMAT_CANVAS").unwrap_or_else(|| "#stratmat");
    let matches = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .query_selector_all(selector)?;

    #[cfg(debug_assertions)]
    console::log_1(&format!("stratmat init: found {} canvas(es)", matches.length()).into());
    let args = match matches.length() {
        0 => Args::parse(),
        1 => {
            let canvas: web_sys::HtmlCanvasElement =
                matches.get(0).unwrap().dyn_into().map_err(|elem| {
                    format!("stratmat requires a <canvas>, not a <{}>", elem.node_name())
                })?;
            let dataset = canvas.dataset();
            let keys = js_sys::Reflect::own_keys(&dataset)?;
            // Arg 0 is the "process name"
            let mut args = vec!["".to_owned()];
            for key in keys.iter() {
                if let Some(name) = key
                    .as_string()
                    .unwrap()
                    .to_case(Case::Kebab)
                    .strip_prefix("stratmat-")
                {
                    args.push(format!("--{name}"));
                    args.push(js_sys::Reflect::get(&dataset, &key)?.as_string().unwrap());
                }
            }
            console::log_1(&format!("stratmat init: args: {:?}", args).into());
            Args::try_parse_from(args).map_err(|e| format!("invalid arguments: {e}"))?
        }
        _ => {
            return Err("multiple elements match selector '{CANVAS}'".into());
        }
    };

    #[cfg(feature = "egui")]
    let primary_window = Window {
        title: "Stratmat".into(),
        canvas: Some(selector.to_string()),
        fit_canvas_to_parent: true,
        prevent_default_event_handling: false,
        ..default()
    };

    start(
        args,
        #[cfg(feature = "egui")]
        primary_window,
    )
    .map_err(|e| JsValue::from_str(&format!("{e}")))
}
