use bevy::{
    ecs::{
        component::ComponentId,
        system::{SystemId, SystemParam, SystemParamItem, SystemState},
        world::DeferredWorld,
    },
    prelude::*,
    utils::{Entry, HashMap},
};
use bevy_egui::{
    egui::{self, Ui},
    EguiContexts,
};

/*
pub struct Widget<'a, In: 'a> {
    pub ui: &'a mut Ui,
    pub id: Entity,
    pub input: In,
}

impl SystemInput for Widget {
    type Param<'i> = Widget<'i, In>
    type Inner<'i> = ;

    fn wrap(this: Self::Inner<'_>) -> Self::Param<'_> {
        todo!()
    }
}
    */

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
        let mut cached_state = states.take(world, id);
        let resp = S::run_with(world, &mut cached_state, ui, id, args);
        cached_state.apply(world);
        states.insert(id, cached_state);

        resp
    })
}

pub struct WidgetWith<'a, A> {
    target: Entity,
    ui: &'a mut Ui,
    args: A,
}

pub type Widget<'a> = WidgetWith<'a, ()>;

impl<'a, A> SystemInput for WidgetWith<'a, A> {
    type Param<'i> = WidgetWith<'i, A>;
    type Inner<'i> = (Entity, (&'i mut Ui, A));

    fn wrap<'i>((target, (ui, args)): Self::Inner<'i>) -> Self::Param<'i> {
        Self { target, ui, args }
    }
}

/// A UI widget may have multiple instances.
/// We need to ensure the local state of these instances is not shared.
/// This hashmap allows us to dynamically store instance states.
#[derive(Resource, Default)]
struct StateInstances<S: WidgetSystem + 'static> {
    instances: HashMap<Entity, Instance<SystemState<S>>>,
}

enum Instance<S> {
    Stored(S),
    InUse,
}

impl<S: WidgetSystem + 'static> StateInstances<S> {
    fn take(&mut self, world: &mut World, id: Entity) -> SystemState<S> {
        match self.instances.entry(id) {
            Entry::Occupied(mut entry) => {
                let mut swap = Instance::InUse;
                std::mem::swap(&mut swap, entry.get_mut());
                match swap {
                    Instance::Stored(s) => s,
                    Instance::InUse => {
                        panic!("WidgetSystem {} is re-entrant!", std::any::type_name::<S>())
                    }
                }
            }
            Entry::Vacant(entry) => {
                debug!(
                    "registering SystemState for Widget {id:?} of type {}",
                    std::any::type_name::<S>()
                );
                entry.insert(Instance::InUse);
                SystemState::new(world)
            }
        }
    }

    fn insert(&mut self, id: Entity, t: SystemState<S>) {
        self.instances.insert(id, Instance::Stored(t));
    }
}
