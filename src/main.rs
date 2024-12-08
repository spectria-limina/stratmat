#![allow(dead_code)]

use std::path::{Path, PathBuf};

use avian2d::prelude::*;
use bevy::render::RenderPlugin;
use bevy::winit::WinitSettings;
use bevy::{log::LogPlugin, prelude::*};
use bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_vector_shapes::prelude::*;
use clap::{ArgAction, Parser as _};

#[cfg(target_arch = "wasm32")]
use {wasm_bindgen::prelude::*, web_sys::HtmlCanvasElement};

#[cfg(test)]
mod testing;

mod arena;
mod asset;
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
    #[clap(long, short)]
    asset_root: Option<PathBuf>,
    #[clap(long, short)]
    log_filter: Option<String>,
    #[cfg(target_arch = "wasm32")]
    #[clap(long, action = ArgAction::Set, default_value_t = false)]
    offscreen_canvas: bool,
}

fn start(args: Args, primary_window: Window) -> eyre::Result<()> {
    let mut app = App::new();

    if let Some(ref path) = args.asset_root {
        target_setup(&mut app, path);
    }

    let mut log_plugin = LogPlugin::default();
    if let Some(ref filter) = args.log_filter {
        log_plugin.filter = filter.clone();
    }

    let mut render_plugin = RenderPlugin::default();
    #[cfg(target_arch = "wasm32")]
    if args.offscreen_canvas {
        render_plugin.offscreen_canvas = true;
    }

    app.insert_resource(args.clone())
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(primary_window),
                    ..default()
                })
                .set(log_plugin)
                .set(AssetPlugin {
                    meta_check: bevy::asset::AssetMetaCheck::Never,
                    ..default()
                })
                .set(render_plugin),
        )
        .add_plugins(EguiPlugin)
        .add_plugins(Shape2dPlugin::default())
        .add_plugins(
            PhysicsPlugins::default(), /* FIXME: Re-disable once Jondolf/avian#571 is fixed.
                                          .disable::<IntegratorPlugin>()
                                          .disable::<SolverPlugin>()
                                          .disable::<SleepingPlugin>(),
                                       */
        )
        .add_plugins(asset::lifecycle::plugin())
        .add_plugins(color::plugin())
        .add_plugins(cursor::plugin())
        .add_plugins(waymark::plugin())
        .add_plugins(waymark::window::plugin())
        .add_plugins(arena::plugin())
        .add_plugins(arena::menu::plugin())
        .insert_resource(WinitSettings::desktop_app());

    if args.debug_inspector {
        app.add_plugins(WorldInspectorPlugin::new());
    }
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

    app.world_mut().spawn((
        Name::new("Primary Camera"),
        PrimaryCamera,
        Camera2d,
        OrthographicProjection::default_2d(),
    ));

    app.run();
    Ok(())
}

#[derive(Component, Reflect, Debug, Copy, Clone)]
struct PrimaryCamera;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eyre::Result<()> {
    let primary_window = Window {
        title: "Stratmat".into(),
        ..default()
    };
    start(Args::parse(), primary_window)
}

#[cfg(not(target_arch = "wasm32"))]
fn target_setup(app: &mut App, path: &Path) {
    use bevy::asset::io::{file::FileAssetReader, AssetSource, AssetSourceId};
    let path = path.to_owned();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSource::build().with_reader(move || Box::new(FileAssetReader::new(path.clone()))),
    );
}

// on the web. So work around that a bit.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(main)]
fn main() -> Result<(), JsValue> {
    use bevy::window::WindowResolution;
    use convert_case::{Case, Casing};
    use web_sys::console;
    console::log_1(&"stratmat init: initializing...".into());

    let selector = option_env!("STRATMAT_CANVAS").unwrap_or_else(|| "#stratmat");
    let matches = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .query_selector_all(selector)?;

    let mut resolution = WindowResolution::default();

    #[cfg(debug_assertions)]
    console::log_1(&format!("stratmat init: found {} canvas(es)", matches.length()).into());
    let args = match matches.length() {
        0 => Args::parse(),
        1 => {
            let canvas: HtmlCanvasElement = matches.get(0).unwrap().dyn_into().map_err(|elem| {
                format!("stratmat requires a <canvas>, not a <{}>", elem.node_name())
            })?;
            resolution.set_physical_resolution(canvas.width(), canvas.height());
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

    let primary_window = Window {
        title: "Stratmat".into(),
        canvas: Some(selector.to_string()),
        fit_canvas_to_parent: false,
        prevent_default_event_handling: false,
        ..default()
    };

    start(args, primary_window).map_err(|e| JsValue::from_str(&format!("{e}")))
}

#[cfg(target_arch = "wasm32")]
fn target_setup(app: &mut App, path: &Path) {
    use bevy::asset::io::{wasm::HttpWasmAssetReader, AssetSource, AssetSourceId};
    let path = path.to_owned();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSource::build().with_reader(move || Box::new(HttpWasmAssetReader::new(path.clone()))),
    );
    app.add_systems(PostUpdate, add_extra_canvas);
}

#[cfg(target_arch = "wasm32")]
fn add_extra_canvas(
    q: Query<&Window>,
    camera_q: Query<(&Camera, &OrthographicProjection)>,
    arena: Option<Single<(), With<arena::Arena>>>,
    mut commands: Commands,
) {
    use bevy::{
        render::camera::RenderTarget,
        window::{WindowRef, WindowResolution},
    };

    // Wait until the arena spawns to get the projection.
    if arena.is_none() {
        return;
    }

    const EXTRA_SELECTOR: &str = "#stratmat-extra";
    let Ok(Some(node)) = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .query_selector(EXTRA_SELECTOR)
    else {
        return;
    };

    let Some(canvas): Option<&HtmlCanvasElement> = node.dyn_ref() else {
        return;
    };

    if !q.iter().any(|w| w.canvas == Some(EXTRA_SELECTOR.into())) {
        info!(
            "Second canvas detected. Creating new {}x{} window and spawning new camera.",
            canvas.width(),
            canvas.height()
        );

        let window_id = commands
            .spawn(Window {
                canvas: Some(EXTRA_SELECTOR.into()),
                fit_canvas_to_parent: false,
                prevent_default_event_handling: false,
                resolution: WindowResolution::new(canvas.width() as f32, canvas.height() as f32),
                ..default()
            })
            .id();

        let (cam, proj) = camera_q.single();
        let (mut cam, mut proj) = (cam.clone(), proj.clone());
        cam.target = RenderTarget::Window(WindowRef::Entity(window_id));
        proj.scale /= 2.0;
        proj.viewport_origin = Vec2::new(0.25, 0.85);

        commands.spawn((Name::new("Secondary Camera"), Camera2d, cam, proj));
    }
}
