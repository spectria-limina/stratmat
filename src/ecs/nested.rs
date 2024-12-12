use std::{any::Any, borrow::Cow, marker::PhantomData};

use bevy::{
    ecs::{
        component::ComponentId,
        query::Access,
        world::{unsafe_world_cell::UnsafeWorldCell, DeferredWorld},
    },
    prelude::*,
    ptr::OwningPtr,
};
use derive_where::derive_where;

use super::*;

pub type ArgInner<'a, Arg> = <Arg as SystemInput>::Inner<'a>;
pub type ArgParam<'a, Arg> = <Arg as SystemInput>::Param<'a>;
pub type NestedSystemArg<'a, Data, Arg> = NestedSystemArgInner<'a, Data, ArgInner<'a, Arg>>;
pub type NestedSystemArgInner<'a, Data, ArgInner> = (&'a mut NestedSystem<'a>, Data, ArgInner);

pub trait HasInnerArg {
    type InnerArg: SystemInput;
}

impl SystemInput for &mut NestedSystem<'_> {
    type Param<'i> = &'i mut NestedSystem<'i>;
    type Inner<'i> = NestedSystemArg<'i, (), ()>;

    fn wrap((ns, _, _): Self::Inner<'_>) -> Self::Param<'_> { ns }
}
impl HasInnerArg for &mut NestedSystem<'_> {
    type InnerArg = ();
}

pub struct NestedWithArg<'a, Arg: SystemInput>(pub &'a mut NestedSystem<'a>, pub Arg);

impl<Arg: SystemInput> SystemInput for NestedWithArg<'_, Arg> {
    type Param<'i> = NestedWithArg<'i, ArgParam<'i, Arg>>;
    type Inner<'i> = NestedSystemArg<'i, (), Arg>;

    fn wrap((ns, _, arg): Self::Inner<'_>) -> Self::Param<'_> { NestedWithArg(ns, Arg::wrap(arg)) }
}
impl<Arg: SystemInput> HasInnerArg for NestedWithArg<'_, Arg> {
    type InnerArg = Arg;
}

pub struct NestedWithData<'a, Data>(pub &'a mut NestedSystem<'a>, pub Data);

impl<Data> SystemInput for NestedWithData<'_, Data> {
    type Param<'i> = NestedWithData<'i, Data>;
    type Inner<'i> = NestedSystemArg<'i, Data, ()>;

    fn wrap((ns, data, _): Self::Inner<'_>) -> Self::Param<'_> { NestedWithData(ns, data) }
}
impl<Data> HasInnerArg for NestedWithData<'_, Data> {
    type InnerArg = ();
}

pub struct NestedWith<'a, Data, Arg: SystemInput>(pub &'a mut NestedSystem<'a>, pub Data, pub Arg);

impl<Data, Arg: SystemInput> SystemInput for NestedWith<'_, Data, Arg> {
    type Param<'i> = NestedWith<'i, Data, ArgParam<'i, Arg>>;
    type Inner<'i> = NestedSystemArg<'i, Data, Arg>;

    fn wrap((ns, data, arg): Self::Inner<'_>) -> Self::Param<'_> {
        NestedWith(ns, data, Arg::wrap(arg))
    }
}
impl<Data, Arg: SystemInput> HasInnerArg for NestedWith<'_, Data, Arg> {
    type InnerArg = Arg;
}


struct SystemWithData<Sys, Data, Arg> {
    sys: Sys,
    data: Data,
    _ph: PhantomData<fn(Arg)>,
}

impl<Sys, Data, Arg> SystemWithData<Sys, Data, Arg> {
    fn new(sys: Sys, data: Data) -> Self {
        Self {
            sys,
            data,
            _ph: PhantomData,
        }
    }
}

pub trait DynNestedSystem: Send + Sync {
    fn queue_deferred(&mut self, world: DeferredWorld);
    fn name(&self) -> Cow<'static, str>;
    fn update_archetype_component_access(&mut self, world: UnsafeWorldCell<'_>);
    fn component_access(&self) -> &Access<ComponentId>;
    // arg MUST be the ArgInner type.
    // the return type is always the Out type.
    //
    // INVARIANT: The pointer must be safe to read with the correct argument type.
    unsafe fn run(
        &mut self,
        nested: &mut NestedSystem<'_>,
        inner_arg: OwningPtr<'_>,
    ) -> Box<dyn Any>;
}

impl<Sys, Data, Arg: SystemInput> DynNestedSystem for SystemWithData<Sys, Data, Arg>
where
    Sys: System,
    <Sys as System>::In: for<'a> SystemInput<Inner<'a> = NestedSystemArg<'a, Data, Arg>>,
    Data: Clone + Send + Sync,
{
    fn name(&self) -> Cow<'static, str> { self.sys.name() }
    fn update_archetype_component_access(&mut self, world: UnsafeWorldCell<'_>) {
        self.sys.update_archetype_component_access(world);
    }
    fn queue_deferred(&mut self, world: DeferredWorld) { self.sys.queue_deferred(world); }
    fn component_access(&self) -> &Access<ComponentId> { self.sys.component_access() }

    unsafe fn run(
        &mut self,
        nested: &mut NestedSystem<'_>,
        inner_arg: OwningPtr<'_>,
    ) -> Box<dyn Any> {
        nested.reborrow_scope(move |nested| {
            let world = nested.world;
            // SAFETY: This is guaranteed safe by our only caller
            let input: SystemIn<Sys> = (nested, self.data.clone(), unsafe { inner_arg.read() });
            let out = unsafe { self.sys.run_unsafe(input, world) };
            unsafe {
                self.sys.queue_deferred(world.into_deferred());
            }
            Box::new(out)
        })
    }
}

type CachedSystem = Cached<Box<dyn DynNestedSystem>>;

#[derive(Resource, Default)]
pub struct NestedSystemRegistry {
    store: Vec<CachedSystem>,
}

impl NestedSystemRegistry {
    pub fn new() -> Self { default() }

    pub fn register<Sys, In, Out, Marker>(
        world: &mut World,
        s: Sys,
    ) -> NestedSystemId<<In as HasInnerArg>::InnerArg, Out>
    where
        Sys: IntoSystem<In, Out, Marker>,
        In: HasInnerArg<InnerArg: 'static>,
        for<'a> In: SystemInput<Inner<'a> = NestedSystemArg<'a, (), <In as HasInnerArg>::InnerArg>>
            + 'static,
        Out: 'static,
    {
        Self::register_with_data(world, s, ())
    }

    pub fn register_with_data<Sys, In, Data, Out, Marker>(
        world: &mut World,
        s: Sys,
        data: Data,
    ) -> NestedSystemId<<In as HasInnerArg>::InnerArg, Out>
    where
        Sys: IntoSystem<In, Out, Marker>,
        In: HasInnerArg<InnerArg: 'static>,
        for<'a> In: SystemInput<Inner<'a> = NestedSystemArg<'a, Data, <In as HasInnerArg>::InnerArg>>
            + 'static,
        Data: Clone + Send + Sync + 'static,
        Out: 'static,
    {
        let mut sys = IntoSystem::into_system(s);
        sys.initialize(world);
        let mut registry = world.resource_mut::<NestedSystemRegistry>();
        let boxed: Box<dyn DynNestedSystem> =
            Box::new(SystemWithData::<_, Data, <In as HasInnerArg>::InnerArg>::new(sys, data));
        registry.store.push(Cached::new(boxed));
        NestedSystemId(registry.store.len() - 1, PhantomData)
    }
}

#[derive_where(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
pub struct NestedSystemId<Arg = (), Out = ()>(usize, PhantomData<fn(Arg) -> Out>);
// SAFETY: It's just phantom data
unsafe impl<Arg, Out> Send for NestedSystemId<Arg, Out> {}
unsafe impl<Arg, Out> Sync for NestedSystemId<Arg, Out> {}

pub struct NestedSystem<'w> {
    accesses: &'w mut Vec<(String, Access<ComponentId>)>,
    world: UnsafeWorldCell<'w>,
    registry: &'w mut NestedSystemRegistry,
}

impl NestedSystem<'_> {
    pub fn reborrow_scope<F, R>(&mut self, f: F) -> R
    where
        for<'i> F: FnOnce(&'i mut NestedSystem<'i>) -> R,
    {
        let mut reborrowed = NestedSystem {
            accesses: self.accesses,
            world: self.world,
            registry: self.registry,
        };
        f(&mut reborrowed)
    }
    pub fn scope<F, R>(world: &mut World, f: F) -> R
    where
        F: for<'a> FnOnce(&'a mut NestedSystem<'a>) -> R,
    {
        world.resource_scope(
            |world: &mut World, mut registry: Mut<NestedSystemRegistry>| {
                let mut accesses = vec![];
                let mut this = NestedSystem {
                    accesses: &mut accesses,
                    world: world.as_unsafe_world_cell(),
                    registry: &mut registry,
                };
                f(&mut this)
            },
        )
    }

    #[track_caller]
    pub fn run_nested<Out: 'static>(&mut self, s: NestedSystemId<(), Out>) -> Out {
        self.run_nested_with(s, ())
    }

    #[track_caller]
    pub fn run_nested_with<Arg: SystemInput + 'static, Out: 'static>(
        &mut self,
        s: NestedSystemId<Arg, Out>,
        arg: ArgInner<Arg>,
    ) -> Out {
        let Some(mut sys) = self
            .registry
            .store
            .get_mut(s.0)
            .unwrap_or_else(|| panic!("Invalid NestedSystemId {}", s.0))
            .take()
        else {
            panic!("NestedSystemId {} is (indirectly?) calling itself", s.0);
        };
        let name = sys.name();

        sys.update_archetype_component_access(self.world);
        let new_access = sys.component_access();
        debug!("Trying to run nested system {name} with component_access {new_access:#?}",);
        let conflicts = self
            .accesses
            .iter()
            .filter(|(_name, access)| !new_access.is_compatible(access))
            .cloned()
            .map(|(name, access)| AccessDiags::new(name, access))
            .collect::<Vec<_>>();
        if !conflicts.is_empty() {
            diagnose_conflicts(
                self.world.components(),
                AccessDiags::new(name.to_string(), new_access.clone()),
                conflicts,
            );
            panic!(
                "{name} cannot run as a nested system due to data access conflicts with systems \
                 up the call stack"
            );
        };

        self.accesses
            .push((sys.name().to_string(), new_access.clone()));
        // SAFETY: The NestedSystemId tells us that arg is the correct type.
        let out = OwningPtr::make(arg, |ptr| unsafe { sys.run(self, ptr) });
        // SAFETY: The only thing we're touching is the command queue,
        //         we never let any other caller touch that.
        unsafe {
            sys.queue_deferred(self.world.into_deferred());
        }
        self.registry.store[s.0] = Cached::Stored(sys);
        // FIXME: Do we need to poison/abort if a panic comes through here? Figure that out.
        // self.accesses.pop();
        match out.downcast::<Out>() {
            Ok(out) => *out,
            Err(_) => panic!(
                "Nested system {name} gave us the wrong output type. Expected {}. Yikes!",
                std::any::type_name::<Out>()
            ),
        }
    }
}

pub trait NestedSystemExts {
    fn run_nested<Out: 'static>(&mut self, s: NestedSystemId<(), Out>) -> Out;
    fn run_nested_with<Arg: SystemInput + 'static, Out: 'static>(
        &mut self,
        s: NestedSystemId<Arg, Out>,
        arg: ArgInner<Arg>,
    ) -> Out;

    fn register_nested<Sys, In, Out, Marker>(
        &mut self,
        s: Sys,
    ) -> NestedSystemId<<In as HasInnerArg>::InnerArg, Out>
    where
        Sys: IntoSystem<In, Out, Marker>,
        In: HasInnerArg<InnerArg: 'static>,
        for<'a> In: SystemInput<Inner<'a> = NestedSystemArg<'a, (), <In as HasInnerArg>::InnerArg>>
            + 'static,
        Out: 'static;
    fn register_nested_with_data<Sys, In, Data, Out, Marker>(
        &mut self,
        s: Sys,
        data: Data,
    ) -> NestedSystemId<<In as HasInnerArg>::InnerArg, Out>
    where
        Sys: IntoSystem<In, Out, Marker>,
        In: HasInnerArg<InnerArg: 'static>,
        for<'a> In: SystemInput<Inner<'a> = NestedSystemArg<'a, Data, <In as HasInnerArg>::InnerArg>>
            + 'static,
        Data: Clone + Send + Sync + 'static,
        Out: 'static;
}

impl NestedSystemExts for World {
    fn run_nested<Out: 'static>(&mut self, s: NestedSystemId<(), Out>) -> Out {
        NestedSystem::scope(self, |nested| nested.run_nested(s))
    }
    fn run_nested_with<Arg: SystemInput + 'static, Out: 'static>(
        &mut self,
        s: NestedSystemId<Arg, Out>,
        arg: ArgInner<Arg>,
    ) -> Out {
        NestedSystem::scope(self, |nested| nested.run_nested_with(s, arg))
    }

    fn register_nested<Sys, In, Out, Marker>(
        &mut self,
        s: Sys,
    ) -> NestedSystemId<<In as HasInnerArg>::InnerArg, Out>
    where
        Sys: IntoSystem<In, Out, Marker>,
        In: HasInnerArg<InnerArg: 'static>,
        for<'a> In: SystemInput<Inner<'a> = NestedSystemArg<'a, (), <In as HasInnerArg>::InnerArg>>
            + 'static,
        Out: 'static,
    {
        NestedSystemRegistry::register(self, s)
    }

    fn register_nested_with_data<Sys, In, Data, Out, Marker>(
        &mut self,
        s: Sys,
        data: Data,
    ) -> NestedSystemId<<In as HasInnerArg>::InnerArg, Out>
    where
        Sys: IntoSystem<In, Out, Marker>,
        In: HasInnerArg<InnerArg: 'static>,
        for<'a> In: SystemInput<Inner<'a> = NestedSystemArg<'a, Data, <In as HasInnerArg>::InnerArg>>
            + 'static,
        Data: Clone + Send + Sync + 'static,
        Out: 'static,
    {
        NestedSystemRegistry::register_with_data(self, s, data)
    }
}

#[cfg(test)]
mod test {
    use std::f32::consts::PI;

    use bevy::log::LogPlugin;

    use super::{super::named, *};

    #[derive(Component, Default, Debug)]
    struct C1;

    #[derive(Component, Default, Debug)]
    struct C2;

    #[derive(Component, Default, Debug)]
    struct C3;

    #[derive(Resource, Default, Debug)]
    struct R1;
    #[derive(Resource, Default, Debug)]
    struct R2;

    #[test]
    #[should_panic]
    fn test_nested_system_basic() {
        let mut app = App::new();
        app.add_plugins(LogPlugin::default());

        app.init_resource::<NestedSystemRegistry>();
        app.world_mut().spawn((C1, C2, C3));

        let wr_c1_c3 = |_ns: &mut NestedSystem, _q: Query<(&mut C1, &C3)>| {
            error!("read/write conflict on C1... undefined behaviour... 😔");
        };
        let id_wr_c1_c3: NestedSystemId =
            NestedSystemRegistry::register(app.world_mut(), named!(wr_c1_c3));
        let rw_c2_c3 = move |NestedWithArg(_ns, In(arg)): NestedWithArg<In<u32>>,
                             _q: Query<(&C2, &mut C3)>| {
            info!("rw_c2_c3");
            assert_eq!(arg, 1728);
            "hi mom!"
        };
        let id_rw_c2_c3: NestedSystemId<In<u32>, &str> =
            NestedSystemRegistry::register(app.world_mut(), named!(rw_c2_c3));
        let r_c1_again = move |ns: &mut NestedSystem, _q: Query<&C1>| {
            info!("r_c1_again");
            assert_eq!(ns.run_nested_with(id_rw_c2_c3, 1728), "hi mom!");
            ns.run_nested(id_wr_c1_c3);
        };
        let id_r_c1_again = NestedSystemRegistry::register(app.world_mut(), named!(r_c1_again));
        let r_c1 = move |NestedWithData(ns, data): NestedWithData<f32>, _q: Query<&C1>| {
            info!("r_c1");
            assert_eq!(data, PI);
            ns.run_nested(id_r_c1_again);
        };
        let id_r_c1 = NestedSystemRegistry::register_with_data(app.world_mut(), named!(r_c1), PI);

        NestedSystem::scope(app.world_mut(), |ns| ns.run_nested(id_r_c1))
    }

    #[test]
    #[should_panic]
    fn test_nested_system_broad_conflicts() {
        let mut app = App::new();
        // This sure is a way to init logging.
        app.add_plugins(LogPlugin::default());

        app.init_resource::<NestedSystemRegistry>();
        app.init_resource::<R1>();
        app.init_resource::<R2>();
        app.world_mut().spawn((C1, C2, C3));
        app.world_mut().spawn((C1, C2, C3));
        app.world_mut().spawn((C1, C2, C3));
        app.world_mut().spawn((C1, C2, C3));

        let rsr_wac =
            |_ns: &mut NestedSystem, _r1: Res<R1>, _r2: Res<R2>, _wsc: Query<EntityMut>| {
                error!("... undefined behaviour... 😔");
            };
        let id_rsr_wac = NestedSystemRegistry::register(app.world_mut(), named!(rsr_wac));
        let rnr_rac = move |ns: &mut NestedSystem, _rac: Query<EntityRef>| {
            info!("rne_rac");
            ns.run_nested(id_rsr_wac);
        };
        let id_rnr_rac = NestedSystemRegistry::register(app.world_mut(), named!(rnr_rac));
        let wsr_rsc = move |ns: &mut NestedSystem, _wsr: ResMut<R1>, _rsc: Query<&C1>| {
            info!("r_c1_again");
            ns.run_nested(id_rnr_rac);
        };
        let id_wsr_rsc = NestedSystemRegistry::register(app.world_mut(), named!(wsr_rsc));

        NestedSystem::scope(app.world_mut(), |ns| ns.run_nested(id_wsr_rsc))
    }
}
