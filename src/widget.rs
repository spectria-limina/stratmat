use bevy::{
    ecs::system::{SystemParamItem, SystemState},
    prelude::*,
};
use bevy_egui::{
    egui::{self, Ui},
    EguiContexts,
};

use crate::ecs::{NestedSystem, NestedSystemId};


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

pub struct WidgetIn<'a>(pub Entity, pub &'a mut Ui);

pub struct WidgethWithIn<'a, A>(pub Entity, pub &'a mut Ui, pub A);

impl SystemInput for WidgetIn<'_> {
    type Param<'i> = WidgetIn<'i>;
    type Inner<'i> = (Entity, &'i mut Ui);

    fn wrap((id, ui): Self::Inner<'_>) -> Self::Param<'_> { WidgetIn(id, ui) }
}

impl<A> SystemInput for WidgethWithIn<'_, A> {
    type Param<'i> = WidgethWithIn<'i, A>;
    type Inner<'i> = (Entity, (&'i mut Ui, A));

    fn wrap((id, (ui, arg)): Self::Inner<'_>) -> Self::Param<'_> { WidgethWithIn(id, ui, arg) }
}

#[derive(Debug, Copy, Clone)]
#[derive(Component)]
pub struct WidgetRegistration(NestedSystemId<InMut<'static, Ui>>);

impl WidgetRegistration {
    pub fn show(&self, nested: &mut NestedSystem, ui: &mut Ui) {
        nested.run_nested_with(self.0, ui)
    }
}

#[derive(Debug, Copy, Clone, Component)]

pub struct HasWidget {}
