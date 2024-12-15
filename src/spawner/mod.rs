use std::{any::type_name, borrow::Cow, fmt::Debug, marker::PhantomData};

use bevy::{
    ecs::{component::ComponentId, system::EntityCommands, world::DeferredWorld},
    picking::{
        backend::{HitData, PointerHits},
        pointer::PointerId,
        PickSet,
    },
    prelude::*,
};
#[cfg(feature = "egui")]
use bevy_egui::{self, egui, EguiUserTextures};
use itertools::Itertools;

use crate::{
    ecs::{EntityExts, EntityExtsOf, NestedSystemExts},
    image::Image,
    widget::{widget, InitWidget, WidgetCtx, WidgetSystemId},
};

#[cfg(feature = "egui")]
mod panel_egui;
pub mod panel {
    #[cfg(feature = "egui")]
    pub use super::panel_egui::*;
}
#[cfg(all(feature = "egui", test))]
mod test_egui;
#[cfg(test)]
pub mod test {
    #[cfg(feature = "egui")]
    pub use super::test_egui::*;
}

/// The alpha (out of 255) of an enabled waymark spawner widget.
const SPAWNER_ALPHA: u8 = 230;
/// The alpha (out of 255) of a disabled waymark spawner widget.
const SPAWNER_DISABLED_ALPHA: u8 = 25;

/// An entity that can be spawned.
pub trait Spawnable: Component + Reflect + TypePath + Clone + PartialEq + Debug + Ord {
    const UNIQUE: bool;

    fn size() -> Vec2;
    fn sep() -> Vec2;

    fn spawner_name(&self) -> Cow<'static, str>;
    fn texture_handle(&self, asset_server: &AssetServer) -> Handle<Image>;
    fn insert(&self, entity: &mut EntityCommands);
}

/// An entity that can be clicked & dragged to spawn a waymark.
///
/// Rendered using egui, not the normal logic.
#[derive(Debug, Clone, Component)]
#[component(on_add = Spawner::<T>::on_add)]
#[component(on_remove = Spawner::<T>::on_remove)]
#[require(InitWidget(|| widget!()))]
pub struct Spawner<T: Spawnable> {
    pub target: T,
    pub image: Handle<Image>,
    pub size: Vec2,
    pub enabled: bool,
}

#[cfg(feature = "egui")]
#[derive(Debug, Copy, Clone, Component)]
pub struct EguiTextureId(egui::TextureId);

impl<T: Spawnable> Spawner<T> {
    pub fn new(target: T, image: Handle<Image>) -> Self {
        Self {
            target,
            image,
            size: T::size(),
            enabled: true,
        }
    }

    pub fn on_add(mut world: DeferredWorld, id: Entity, _: ComponentId) {
        let spawner = world
            .get_mut::<Self>(id)
            .expect("I was just added!")
            .clone();
        #[cfg(feature = "egui")]
        let texture_id = world
            .resource_mut::<EguiUserTextures>()
            .add_image(spawner.image);
        let mut commands = world.commands();
        let mut entity = commands.entity(id);

        entity
            .insert_if_new(Name::new(format!(
                "Spawner for {}",
                spawner.target.spawner_name(),
            )))
            .on::<Self>()
            .observe(Self::start_drag);
        #[cfg(feature = "egui")]
        entity.insert(EguiTextureId(texture_id));
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

    /// Handle a drag event, spawning a new entity in place of the current entity if
    /// the [Spawner] is enabled.
    ///
    /// Technically what it actually does is, to preserve continuity of the drag event,
    /// replaces this entity with the new waymark, and spawns a new [Spawner] in its place.
    ///
    /// Panics if there is more than one camera.
    pub fn start_drag(
        ev: Trigger<Pointer<DragStart>>,
        spawner_q: Query<(&Spawner<T>, Option<&Parent>)>,
        #[cfg(feature = "egui")] camera_q: Query<(&Camera, &GlobalTransform)>,
        children_q: Query<&mut Children>,
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
        entity.remove::<Self>();
        // This will have no effect if we aren't parented.
        // If we are, we've replaced ourself with the new spawner
        entity.remove_parent();

        spawner.target.insert(&mut entity);

        #[cfg(feature = "egui")]
        {
            let (camera, camera_transform) = camera_q.single();
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
            entity.insert(Transform::from_translation(translation));
        }

        // Forward to the general dragging implementation.
        commands.run_system_cached_with(crate::drag::start_drag, id);
    }

    #[cfg(feature = "egui")]
    pub fn show(
        WidgetCtx { ns: _ns, id, ui }: WidgetCtx,
        spawner_q: Query<(&Spawner<T>, &EguiTextureId)>,
        mut pointer_ev: EventWriter<PointerHits>,
    ) {
        let (spawner, texture_id) = spawner_q
            .get(id)
            .expect("Spawner::show called without a Spawner");
        debug!("Drawing Spawner<{:?}>: {:?}", type_name::<T>(), spawner);
        let resp = ui.add(
            egui::Image::new((
                texture_id.0,
                egui::Vec2::new(spawner.size.x, spawner.size.y),
            ))
            .tint(egui::Color32::from_white_alpha(if spawner.enabled {
                SPAWNER_ALPHA
            } else {
                SPAWNER_DISABLED_ALPHA
            }))
            .sense(egui::Sense::drag()),
        );

        if resp.hovered() {
            let egui::Pos2 { x, y } = resp.hover_pos().unwrap();
            pointer_ev.send(PointerHits::new(
                PointerId::Mouse,
                vec![(id, HitData::new(id, 0.0, Some(Vec3::new(x, y, 0.0)), None))],
                // egui is at depth 1_000_000, we need to be in front of that.
                1_000_001.0,
            ));
        }
    }

    #[cfg(feature = "dom")]
    pub fn show(_: WidgetCtx) { todo!() }
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
