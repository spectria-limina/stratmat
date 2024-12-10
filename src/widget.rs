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

pub struct Widget<'a>(pub Entity, pub &'a mut Ui);

pub struct WidgetWith<'a, A>(pub Entity, pub &'a mut Ui, pub A);

impl SystemInput for Widget<'_> {
    type Param<'i>
        = Widget<'i>
    where
        Self: 'i;
    type Inner<'i>
        = (Entity, &'i mut Ui)
    where
        Self: 'i;

    fn wrap<'i>((id, ui): Self::Inner<'i>) -> Self::Param<'i>
    where
        Self: 'i,
    {
        Widget(id, ui)
    }
}

impl<A> SystemInput for WidgetWith<'_, A> {
    type Param<'i>
        = WidgetWith<'i, A>
    where
        Self: 'i;
    type Inner<'i>
        = (Entity, (&'i mut Ui, A))
    where
        Self: 'i;

    fn wrap<'i>((id, (ui, arg)): Self::Inner<'i>) -> Self::Param<'i>
    where
        Self: 'i,
    {
        WidgetWith(id, ui, arg)
    }
}
