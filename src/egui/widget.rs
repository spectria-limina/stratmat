use bevy::{
    ecs::{
        component::ComponentId,
        system::{SystemParamItem, SystemState},
        world::DeferredWorld,
    },
    prelude::*,
};
use bevy_egui::{
    egui::{self, Ui},
    EguiContexts,
};

use crate::ecs::{HasInnerArg, NestedSystemCtx, NestedSystemExts, NestedSystemId};

// TODO: TEST TEST TEST
pub fn egui_contexts_scope<U, F: FnOnce(SystemParamItem<EguiContexts>) -> U>(
    world: &mut World,
    f: F,
) -> U {
    let mut state = SystemState::<EguiContexts>::new(world);
    f(state.get_mut(world))
}

pub fn egui_context(world: &mut World) -> egui::Context {
    egui_contexts_scope(world, |mut contexts| contexts.ctx_mut().clone())
}

pub struct WidgetCtx<'a> {
    pub ns: &'a mut NestedSystemCtx<'a>,
    pub id: Entity,
    pub ui: &'a mut Ui,
}

impl SystemInput for WidgetCtx<'_> {
    type Param<'i> = WidgetCtx<'i>;
    type Inner<'i> = (&'i mut NestedSystemCtx<'i>, Entity, &'i mut Ui);

    fn wrap((ns, id, ui): Self::Inner<'_>) -> Self::Param<'_> { WidgetCtx { ns, id, ui } }
}
impl HasInnerArg for WidgetCtx<'_> {
    type InnerArg = InMut<'static, Ui>;
}

pub type WidgetSystemId = NestedSystemId<InMut<'static, Ui>>;

#[derive(Debug, Copy, Clone)]
#[derive(Component)]
pub struct Widget(WidgetSystemId);

impl Widget {
    pub fn show(&self, nested: &mut NestedSystemCtx, ui: &mut Ui) {
        nested.run_nested_with(self.0, ui)
    }
    pub fn show_world(&self, world: &mut World, ui: &mut Ui) { world.run_nested_with(self.0, ui) }
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
        $crate::egui::widget::InitWidget(
            |world: &mut World, id: Entity| -> $crate::egui::widget::WidgetSystemId {
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
        pub fn show(NestedWith(_ns, _id, InMut(_ui)): NestedWith<Entity, InMut<Ui>>) {
            // do ui stuff here i guess
        }
    }
}

pub struct WidgetPlugin;
impl Plugin for WidgetPlugin {
    fn build(&self, _app: &mut App) { let _ = (); }
}
pub fn plugin() -> WidgetPlugin { WidgetPlugin }
