use bevy::{
    ecs::system::{SystemParamItem, SystemState},
    prelude::*,
};
use bevy_egui::{
    egui::{self, Ui},
    EguiContexts,
};

pub type UiCtx = Ui;

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
