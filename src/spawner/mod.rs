use std::{any::type_name, borrow::Cow, fmt::Debug, marker::PhantomData, path::PathBuf};

use bevy::{
    ecs::{component::ComponentId, system::EntityCommands, world::DeferredWorld},
    picking::{
        backend::{HitData, PointerHits},
        pointer::PointerId,
        PickSet,
    },
    prelude::*,
    transform::systems::{propagate_transforms, sync_simple_transforms},
};
#[cfg(feature = "egui")]
use bevy_egui::{self, EguiUserTextures};
use itertools::Itertools;

#[cfg(feature = "egui")]
use crate::ui::widget::{widget, InitWidget, WidgetCtx, WidgetSystemId};
use crate::{
    arena::Arena,
    ecs::{EntityExts, EntityExtsOf, NestedSystemExts},
    image::{DrawImage, DrawImageKind},
};

#[cfg(feature = "egui")]
mod egui;
#[cfg(feature = "egui")]
pub use egui::*;

#[cfg(feature = "egui")]
mod panel_egui;
pub mod panel {
    #[cfg(feature = "egui")]
    pub use super::panel_egui::*;
}

#[cfg(all(feature = "egui", test))]
mod test_egui;

/// The alpha (out of 255) of an enabled waymark spawner widget.
const SPAWNER_ALPHA: u8 = 230;
/// The alpha (out of 255) of a disabled waymark spawner widget.
const SPAWNER_DISABLED_ALPHA: u8 = 25;

/// An entity that can be spawned.
pub trait Spawnable: Component + Reflect + TypePath + Clone + PartialEq + Debug + Ord {
    const UNIQUE: bool;
    const Z: f32;

    fn size() -> Vec2;
    fn sep() -> Vec2;

    fn spawner_name(&self) -> Cow<'static, str>;
    fn insert(&self, entity: &mut EntityCommands);
}

/// An entity that can be clicked & dragged to spawn a waymark.
///
/// Rendered using egui, not the normal logic.
#[derive(Debug, Clone, Component)]
#[component(on_add = Spawner::<T>::on_add)]
#[component(on_remove = Spawner::<T>::on_remove)]
#[cfg_attr(feature = "egui", require(InitWidget(|| widget!())))]
pub struct Spawner<T: Spawnable> {
    pub target: T,
    pub path: PathBuf,
    pub enabled: bool,
}

impl<T: Spawnable> Spawner<T> {
    pub fn new(target: T, path: PathBuf) -> Self {
        Self {
            target,
            path,
            enabled: true,
        }
    }

    pub fn on_add(mut world: DeferredWorld, id: Entity, _: ComponentId) {
        let spawner = world
            .get_mut::<Self>(id)
            .expect("I was just added!")
            .clone();
        let mut commands = world.commands();
        let mut entity = commands.entity(id);

        entity
            .insert_if_new(Name::new(format!(
                "Spawner for {}",
                spawner.target.spawner_name(),
            )))
            .on::<Self>()
            .observe(Self::start_drag);
        entity.insert(DrawImage::new(spawner.path, T::size(), DrawImageKind::Ui));
    }

    pub fn on_remove(mut world: DeferredWorld, id: Entity, _: ComponentId) {
        world.commands().entity(id).on::<Self>().despawn_children();
    }

    // TODO: TEST TEST TEST
    pub fn update_enabled_state(mut q: Query<&mut Spawner<T>>, target_q: Query<&T>) {
        for mut spawner in &mut q {
            spawner.enabled = !target_q.iter().contains(&spawner.target);
        }
    }

    /// Handle a drag event, spawning a new entity to drag if the [Spawner] is enabled.
    ///
    /// Technically what it actually does is, to preserve continuity of the drag event,
    /// replaces this entity with the new waymark, and spawns a new [Spawner] in its place.
    ///
    /// The new entity will be a child of the current arena.
    ///
    /// Panics if there is more than one camera or arena.
    pub fn start_drag(
        ev: Trigger<Pointer<DragStart>>,
        spawner_q: Query<(&Spawner<T>, Option<&Parent>)>,
        #[cfg(feature = "egui")] camera_q: Single<(&Camera, &GlobalTransform)>,
        children_q: Query<&mut Children>,
        arena_q: Option<Single<Entity, With<Arena>>>,
        mut commands: Commands,
    ) {
        let id = ev.entity();
        debug!("starting drag on spawner {id:?}");
        let Ok((spawner, parent)) = spawner_q.get(id) else {
            debug!("but it doesn't exist");
            return;
        };
        if !spawner.enabled {
            debug!("but it was disabled");
            return;
        }

        let mut new_spawner = commands.spawn(spawner.clone());
        if let Some(parent) = parent {
            new_spawner.set_parent(parent.get());
        }

        let mut entity = commands.entity(id);
        entity.remove::<(Self, Name)>();
        spawner.target.insert(&mut entity);

        #[cfg(feature = "egui")]
        {
            let (camera, camera_transform) = *camera_q;
            let hit_position = ev.hit.position.unwrap().truncate();
            let translation = camera
                .viewport_to_world_2d(camera_transform, hit_position)
                .unwrap()
                .extend(0.0);
            debug!(
                "spawner spawning waymark {:?} at {translation} (from hit position: \
                 {hit_position})",
                spawner.target,
            );
            entity.insert(Transform::from_translation(translation.with_z(T::Z)));
        }

        if let Some(arena_id) = arena_q {
            entity.set_parent(*arena_id);
        } else {
            warn!("Spawner {:?} spawning an entity without a parent", id);
        }

        // Forward to the general dragging implementation.
        commands.run_system_cached_with(crate::drag::start_drag, id);
    }
}

/// Plugin for spawner support
#[derive(Copy, Clone, derive_more::Debug)]
pub struct SpawnerPlugin<Target> {
    #[debug(skip)]
    _phantom: PhantomData<Target>,
}

impl<T> Default for SpawnerPlugin<T> {
    fn default() -> Self {
        Self {
            _phantom: default(),
        }
    }
}

impl<T: Spawnable> Plugin for SpawnerPlugin<T> {
    fn build(&self, app: &mut App) {
        if <T as Spawnable>::UNIQUE {
            app.add_systems(PostUpdate, Spawner::<T>::update_enabled_state);
            #[cfg(feature = "egui")]
            app.add_systems(
                PreUpdate,
                panel::SpawnerPanel::<T>::sort_children.after(PickSet::Last),
            );
        }
    }
}

pub fn plugin<T: Spawnable>() -> SpawnerPlugin<T> { default() }
