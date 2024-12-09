use bevy::{
    ecs::system::{SystemParamItem, SystemState},
    prelude::*,
};
use bevy_egui::{
    egui::{self, Ui},
    EguiContexts,
};

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

pub struct Widget<'a> {
    pub id: Entity,
    pub ui: &'a mut Ui,
}

pub struct WidgetWith<'a, A> {
    pub id: Entity,
    pub ui: &'a mut Ui,
    pub args: A,
}

impl SystemInput for Widget<'_> {
    type Param<'i> = Widget<'i>;
    type Inner<'i> = (Entity, &'i mut Ui);

    fn wrap((id, ui): Self::Inner<'_>) -> Self::Param<'_> {
        Widget { id, ui }
    }
}

impl<A> SystemInput for WidgetWith<'_, A> {
    type Param<'i> = WidgetWith<'i, A>;
    type Inner<'i> = (Entity, (&'i mut Ui, A));

    fn wrap((id, (ui, args)): Self::Inner<'_>) -> Self::Param<'_> {
        WidgetWith { id, ui, args }
    }
}
