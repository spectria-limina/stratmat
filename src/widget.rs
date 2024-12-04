use bevy::{
    ecs::system::{SystemParam, SystemParamItem, SystemState},
    prelude::*,
    utils::HashMap,
};
use bevy_egui::{
    egui::{self, Ui},
    EguiContexts,
};

pub trait WidgetSystem: SystemParam {
    type In;
    type Out;

    fn run(world: &mut World, state: &mut SystemState<Self>, ui: &mut Ui, id: Entity) -> Self::Out
    where
        Self: WidgetSystem<In = ()>,
    {
        Self::run_with(world, state, ui, id, ())
    }

    fn run_with(
        world: &mut World,
        state: &mut SystemState<Self>,
        ui: &mut Ui,
        id: Entity,
        args: Self::In,
    ) -> Self::Out;
}

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

pub fn show<S: 'static + WidgetSystem<In = ()>>(
    world: &mut World,
    ui: &mut Ui,
    id: Entity,
) -> S::Out {
    show_with::<S>(world, ui, id, ())
}

pub fn show_with<S: 'static + WidgetSystem>(
    world: &mut World,
    ui: &mut Ui,
    id: Entity,
    args: S::In,
) -> S::Out {
    // We need to cache `SystemState` to allow for a system's locally tracked state
    if !world.contains_resource::<StateInstances<S>>() {
        // Note, this message should only appear once! If you see it twice in the logs, the function
        // may have been called recursively, and will panic.
        debug!("Init system state {}", std::any::type_name::<S>());
        world.insert_resource(StateInstances::<S> {
            instances: HashMap::new(),
        });
    }
    world.resource_scope(|world, mut states: Mut<StateInstances<S>>| {
        if !states.instances.contains_key(&id) {
            debug!(
                "registering SystemState for Widget {id:?} of type {}",
                std::any::type_name::<S>()
            );
            states.instances.insert(id, SystemState::new(world));
        }
        let cached_state = states.instances.get_mut(&id).unwrap();
        let resp = S::run_with(world, cached_state, ui, id, args);
        cached_state.apply(world);
        resp
    })
}

/// A UI widget may have multiple instances. We need to ensure the local state of these instances is
/// not shared. This hashmap allows us to dynamically store instance states.
#[derive(Resource, Default)]
struct StateInstances<T: WidgetSystem + 'static> {
    instances: HashMap<Entity, SystemState<T>>,
}
