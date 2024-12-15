use bevy::{
    ecs::{
        component::ComponentId,
        system::{SystemParamItem, SystemState},
        world::DeferredWorld,
    },
    prelude::*,
};

use crate::ecs::{HasInnerArg, NestedSystem, NestedSystemExts, NestedSystemId};

#[cfg(feature = "egui")]
mod egui;
#[cfg(feature = "egui")]
pub use egui::*;

#[cfg(feature = "dom")]
mod dom;
#[cfg(feature = "dom")]
pub use dom::*;

pub struct WidgetCtx<'a> {
    pub ns: &'a mut NestedSystem<'a>,
    pub id: Entity,
    pub ui: &'a mut UiCtx,
}

impl SystemInput for WidgetCtx<'_> {
    type Param<'i> = WidgetCtx<'i>;
    type Inner<'i> = (&'i mut NestedSystem<'i>, Entity, &'i mut UiCtx);

    fn wrap((ns, id, ui): Self::Inner<'_>) -> Self::Param<'_> { WidgetCtx { ns, id, ui } }
}
impl HasInnerArg for WidgetCtx<'_> {
    type InnerArg = InMut<'static, UiCtx>;
}

pub type WidgetSystemId = NestedSystemId<InMut<'static, UiCtx>>;

#[derive(Debug, Copy, Clone)]
#[derive(Component)]
pub struct Widget(WidgetSystemId);

impl Widget {
    pub fn show(&self, nested: &mut NestedSystem, ui: &mut UiCtx) {
        nested.run_nested_with(self.0, ui)
    }
    pub fn show_world(&self, world: &mut World, ui: &mut UiCtx) {
        world.run_nested_with(self.0, ui)
    }
}

#[derive(Debug, Copy, Clone, Component)]
#[component(storage = "SparseSet")]
#[component(on_add = Self::init)]

pub struct InitWidget(pub fn(&mut World, Entity) -> WidgetSystemId);

impl InitWidget {
    pub fn init(mut world: DeferredWorld, id: Entity, _: ComponentId) {
        let this = *world.get::<Self>(id).unwrap();
        world.commands().queue(move |world: &mut World| {
            let nested_system_id = this.0(world, id);
            world
                .entity_mut(id)
                .insert(Widget(nested_system_id))
                .remove::<Self>();
        });
    }
}

#[macro_export]
macro_rules! widget {
    () => {
        $crate::widget!(Self::show)
    };
    ($show:path) => {
        $crate::widget::InitWidget(
            |world: &mut World, id: Entity| -> $crate::widget::WidgetSystemId {
                debug!(
                    "Registering widget {:?} {:?} with show function {:?}",
                    id,
                    ::std::any::type_name::<Self>(),
                    ::std::any::type_name_of_val(&$show)
                );
                $crate::ecs::NestedSystemRegistry::register_with_data(world, $show, id)
            },
        )
    };
}
#[allow(unused)]
pub use crate::widget;

#[cfg(test)]
mod test {
    use super::*;
    use crate::ecs::NestedWith;

    #[derive(Component)]
    #[require(InitWidget(|| widget!()))]
    struct Test;

    impl Test {
        pub fn show(NestedWith(_ns, _id, InMut(_ui)): NestedWith<Entity, InMut<UiCtx>>) {
            // do ui stuff here i guess
        }
    }
}

pub struct WidgetPlugin;
impl Plugin for WidgetPlugin {
    fn build(&self, _app: &mut App) { let _ = (); }
}
pub fn plugin() -> WidgetPlugin { WidgetPlugin }
