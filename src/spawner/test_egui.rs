use avian2d::PhysicsPlugins;
use bevy::{
    app::ScheduleRunnerPlugin,
    input::mouse::MouseButtonInput,
    picking::pointer::PointerInput,
    render::{
        settings::{RenderCreation, WgpuSettings},
        RenderPlugin,
    },
    window::{PrimaryWindow, WindowEvent},
    winit::WinitPlugin,
};
use bevy_egui::{egui, EguiPlugin};
use float_eq::assert_float_eq;

use super::*;
use crate::{
    drag,
    ecs::{EntityWorldExts, NestedSystemExts},
    testing::*,
    waymark::Waymark,
    widget::{egui_context, Widget, WidgetSystemId},
};

// TODO: Put this somewhere better lol.
fn log_debug<E: std::fmt::Debug + Event>(mut events: EventReader<E>) {
    for ev in events.read() {
        debug!("{ev:?}");
    }
}

fn observe_debug<E: std::fmt::Debug + Event>(ev: Trigger<E>) {
    debug!("{:?} on {}", ev.event(), ev.entity());
}

#[derive(Default, Resource)]
struct TestWinPos(egui::Pos2);

const SPAWNER_SIZE: f32 = 40.0;

fn draw_test_win<T: Spawnable>(world: &mut World) {
    let ctx = egui_context(world);
    let pos = world.resource::<TestWinPos>().0;
    egui::Area::new("test".into())
        .fixed_pos(pos)
        .show(&ctx, |ui| {
            let mut state = world.query_filtered::<&Widget, With<Spawner<Waymark>>>();
            let widget = *state.single(world);
            widget.show_world(world, ui);
        });
}

fn spawn_test_entities(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Spawner::new(Waymark::A, Waymark::A.asset_path().into()));
    commands.spawn(DragSurfaceBundle::new(Rect::from_center_half_size(
        Vec2::ZERO,
        Vec2::splat(200.0),
    )));
}

// returns the primary window ID and the app itself
fn test_app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(RenderPlugin {
                synchronous_pipeline_compilation: true,
                render_creation: RenderCreation::Automatic(WgpuSettings {
                    backends: None,
                    ..default()
                }),
            })
            .disable::<WinitPlugin>(),
    )
    // Allow for controlled looping & exit.
    .add_plugins(ScheduleRunnerPlugin {
        run_mode: bevy::app::RunMode::Loop { wait: None },
    })
    .add_plugins(PhysicsPlugins::default())
    .add_systems(PreUpdate, forward_window_events)
    .add_plugins(EguiPlugin)
    .add_plugins(drag::plugin())
    .add_plugins(super::plugin::<Waymark>())
    .add_systems(Startup, add_test_camera)
    .add_systems(Startup, spawn_test_entities)
    .add_systems(Update, draw_test_win::<Waymark>)
    .init_resource::<TestWinPos>();
    // Make sure to finalize and to update once to initialize the UI.
    // Don't use app.run() since it'll loop.
    app.finish();
    app.cleanup();
    app.update();

    let mut win_q = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>();
    let primary_window = win_q.single(app.world());
    (app, primary_window)
}

#[test]
fn spawner_drag() {
    let (mut app, _) = test_app();

    let drag = Vec2::splat(50.0);
    let start_pos = Vec2::splat(SPAWNER_SIZE / 2.0);
    let end_pos = start_pos + drag;
    app.world_mut().spawn(MockDrag {
        start_pos,
        end_pos,
        button: MouseButton::Left,
        duration: 10.0,
    });
    app.add_systems(First, MockDrag::update)
        .add_observer(observe_debug::<Pointer<DragStart>>)
        .add_observer(observe_debug::<Pointer<Drag>>)
        .add_observer(observe_debug::<Pointer<DragEnd>>)
        .add_systems(Update, log_debug::<WindowEvent>)
        .add_systems(Update, log_debug::<CursorMoved>)
        .add_systems(Update, log_debug::<MouseButtonInput>)
        .add_systems(Update, log_debug::<PointerHits>)
        .add_systems(Update, log_debug::<PointerInput>)
        .add_systems(First, || {
            debug!("new tick");
        });
    for _ in 0..20 {
        app.update();
    }

    let mut spawner_q = app
        .world_mut()
        .query_filtered::<(), With<Spawner<Waymark>>>();
    spawner_q.single(app.world());

    let mut waymark_q = app
        .world_mut()
        .query_filtered::<&Transform, With<Waymark>>();
    let transform = waymark_q.single(app.world());
    assert_float_eq!(transform.translation.x, end_pos.x, abs <= 0.0001,);
    assert_float_eq!(transform.translation.y, end_pos.y, abs <= 0.0001,);
}

#[cfg(test)]
mod test {
    use avian2d::PhysicsPlugins;
    use bevy::{
        app::ScheduleRunnerPlugin,
        input::mouse::MouseButtonInput,
        picking::pointer::PointerInput,
        render::{
            settings::{RenderCreation, WgpuSettings},
            RenderPlugin,
        },
        window::{PrimaryWindow, WindowEvent},
        winit::WinitPlugin,
    };
    use bevy_egui::{egui, EguiPlugin};
    use float_eq::assert_float_eq;

    use super::*;
    use crate::{
        drag,
        ecs::{self, EntityWorldExts, NestedSystemExts},
        testing::*,
        waymark::Waymark,
        widget::{egui_context, Widget, WidgetSystemId},
    };

    #[derive(Default, Resource)]
    struct TestWinPos(egui::Pos2);

    const SPAWNER_SIZE: f32 = 40.0;

    fn draw_test_win<T: Spawnable>(world: &mut World) {
        let ctx = egui_context(world);
        let pos = world.resource::<TestWinPos>().0;
        egui::Area::new("test".into())
            .fixed_pos(pos)
            .show(&ctx, |ui| {
                let mut state = world.query_filtered::<&Widget, With<Spawner<Waymark>>>();
                let widget = *state.single(world);
                widget.show_world(world, ui);
            });
    }

    fn spawn_test_entities(mut commands: Commands, asset_server: Res<AssetServer>) {
        commands.spawn(Spawner::new(Waymark::A, Waymark::A.asset_path().into()));
        commands.spawn(DragSurfaceBundle::new(Rect::from_center_half_size(
            Vec2::ZERO,
            Vec2::splat(200.0),
        )));
    }

    // returns the primary window ID and the app itself
    fn test_app() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(
            DefaultPlugins
                .set(RenderPlugin {
                    synchronous_pipeline_compilation: true,
                    render_creation: RenderCreation::Automatic(WgpuSettings {
                        backends: None,
                        ..default()
                    }),
                })
                .disable::<WinitPlugin>(),
        )
        // Allow for controlled looping & exit.
        .add_plugins(ScheduleRunnerPlugin {
            run_mode: bevy::app::RunMode::Loop { wait: None },
        })
        .add_plugins(PhysicsPlugins::default())
        .add_systems(PreUpdate, forward_window_events)
        .add_plugins(EguiPlugin)
        .add_plugins(drag::plugin())
        .add_plugins(ecs::plugin())
        .add_plugins(super::plugin::<Waymark>())
        .add_systems(Startup, add_test_camera)
        .add_systems(Startup, spawn_test_entities)
        .add_systems(Update, draw_test_win::<Waymark>)
        .init_resource::<TestWinPos>();
        // Make sure to finalize and to update once to initialize the UI.
        // Don't use app.run() since it'll loop.
        app.finish();
        app.cleanup();
        app.update();

        let mut win_q = app
            .world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>();
        let primary_window = win_q.single(app.world());
        (app, primary_window)
    }

    #[test]
    fn spawner_drag() {
        let (mut app, _) = test_app();

        let drag = Vec2::splat(50.0);
        let start_pos = Vec2::splat(SPAWNER_SIZE / 2.0);
        let end_pos = start_pos + drag;
        app.world_mut().spawn(MockDrag {
            start_pos,
            end_pos,
            button: MouseButton::Left,
            duration: 10.0,
        });
        app.add_systems(First, MockDrag::update)
            .add_observer(observe_debug::<Pointer<DragStart>>)
            .add_observer(observe_debug::<Pointer<Drag>>)
            .add_observer(observe_debug::<Pointer<DragEnd>>)
            .add_systems(Update, log_debug::<WindowEvent>)
            .add_systems(Update, log_debug::<CursorMoved>)
            .add_systems(Update, log_debug::<MouseButtonInput>)
            .add_systems(Update, log_debug::<PointerHits>)
            .add_systems(Update, log_debug::<PointerInput>)
            .add_systems(First, || {
                debug!("new tick");
            });
        for _ in 0..20 {
            app.update();
        }

        let mut spawner_q = app
            .world_mut()
            .query_filtered::<(), With<Spawner<Waymark>>>();
        spawner_q.single(app.world());

        let mut waymark_q = app
            .world_mut()
            .query_filtered::<&Transform, With<Waymark>>();
        let transform = waymark_q.single(app.world());
        assert_float_eq!(transform.translation.x, end_pos.x, abs <= 0.0001,);
        assert_float_eq!(transform.translation.y, end_pos.y, abs <= 0.0001,);
    }
}
