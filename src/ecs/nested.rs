use std::{
    any::{Any, TypeId},
    borrow::Cow,
    fmt::Write,
    marker::PhantomData,
};

use bevy::{
    ecs::{
        archetype::ArchetypeComponentId,
        component::{ComponentId, Components, Tick},
        query::{Access, AccessConflicts},
        schedule::InternedSystemSet,
        world::{unsafe_world_cell::UnsafeWorldCell, DeferredWorld},
    },
    prelude::*,
    ptr::OwningPtr,
};
use derive_more::derive::Display;
use derive_where::derive_where;

use super::Cached;

pub type ArgInner<'a, Arg> = <Arg as SystemInput>::Inner<'a>;
pub type ArgParam<'a, Arg> = <Arg as SystemInput>::Param<'a>;
pub type NestedSystemArg<'a, Data, Arg> = NestedSystemArgInner<'a, Data, ArgInner<'a, Arg>>;
pub type NestedSystemArgInner<'a, Data, ArgInner> = (&'a mut NestedSystem<'a>, Data, ArgInner);

pub trait HasInnerArg {
    type InnerArg: SystemInput;
}

impl<'a> SystemInput for &mut NestedSystem<'a> {
    type Param<'i> = &'i mut NestedSystem<'i>;
    type Inner<'i> = NestedSystemArg<'i, (), ()>;

    fn wrap<'i>((ns, _, _): Self::Inner<'i>) -> Self::Param<'i> { ns }
}
impl<'a> HasInnerArg for &mut NestedSystem<'a> {
    type InnerArg = ();
}

pub struct NestedWithArg<'a, Arg: SystemInput>(&'a mut NestedSystem<'a>, Arg);

impl<'a, Arg: SystemInput> SystemInput for NestedWithArg<'a, Arg> {
    type Param<'i> = NestedWithArg<'i, ArgParam<'i, Arg>>;
    type Inner<'i> = NestedSystemArg<'i, (), Arg>;

    fn wrap<'i>((ns, _, arg): Self::Inner<'i>) -> Self::Param<'i> {
        NestedWithArg(ns, Arg::wrap(arg))
    }
}
impl<'a, Arg: SystemInput> HasInnerArg for NestedWithArg<'a, Arg> {
    type InnerArg = Arg;
}

pub struct NestedWithData<'a, Data>(&'a mut NestedSystem<'a>, Data);

impl<'a, Data> SystemInput for NestedWithData<'a, Data> {
    type Param<'i> = NestedWithData<'i, Data>;
    type Inner<'i> = NestedSystemArg<'i, Data, ()>;

    fn wrap<'i>((ns, data, _): Self::Inner<'i>) -> Self::Param<'i> { NestedWithData(ns, data) }
}
impl<'a, Data> HasInnerArg for NestedWithData<'a, Data> {
    type InnerArg = ();
}

pub struct NestedWith<'a, Data, Arg: SystemInput>(&'a mut NestedSystem<'a>, Data, Arg);

impl<'a, Data, Arg: SystemInput> SystemInput for NestedWith<'a, Data, Arg> {
    type Param<'i> = NestedWith<'i, Data, ArgParam<'i, Arg>>;
    type Inner<'i> = NestedSystemArg<'i, Data, Arg>;

    fn wrap<'i>((ns, data, arg): Self::Inner<'i>) -> Self::Param<'i> {
        NestedWith(ns, data, Arg::wrap(arg))
    }
}
impl<'a, Data, Arg: SystemInput> HasInnerArg for NestedWith<'a, Data, Arg> {
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
    unsafe fn run<'w>(
        &mut self,
        nested: &mut NestedSystem<'w>,
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

    unsafe fn run<'w>(
        &mut self,
        nested: &mut NestedSystem<'w>,
        inner_arg: OwningPtr<'_>,
    ) -> Box<dyn Any> {
        nested.reborrow_scope(move |nested| {
            let world = nested.world;
            let input: SystemIn<Sys> = (nested, self.data.clone(), inner_arg.read());
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
pub struct NestedSystemId<Arg: SystemInput + 'static = (), Out: 'static = ()>(
    usize,
    PhantomData<fn(Arg) -> Out>,
);

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
            accesses: &mut self.accesses,
            world: self.world,
            registry: &mut self.registry,
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

#[derive(Debug, Display, Copy, Clone)]
pub enum BroadAccess {
    #[display("")]
    None,
    #[display("Some")]
    Some,
    #[display("**ALL**")]
    All,
}

#[derive(Debug, Display, Copy, Clone)]
pub enum BroadAccessKind {
    #[display("Read Components")]
    ReadComponents,
    #[display("Write Components")]
    WriteComponents,
    #[display("Read Resources")]
    ReadResources,
    #[display("Write Resources")]
    WriteResources,
}

#[derive(Debug, Display, Copy, Clone)]
pub enum NarrowAccess {
    #[display("")]
    None,
    #[display("Read")]
    Read,
    #[display("**WRITE**")]
    Write,
}

#[derive(Deref, Debug, Clone)]
pub struct AccessDiags {
    name: String,
    #[deref]
    access: Access<ComponentId>,
}

impl AccessDiags {
    pub fn new(name: String, access: Access<ComponentId>) -> Self { Self { name, access } }

    pub fn broad(&self, kind: BroadAccessKind) -> BroadAccess {
        type Pred = fn(&Access<ComponentId>) -> bool;
        fn on(this: &AccessDiags, any: Pred, all: Pred) -> BroadAccess {
            if all(&this) {
                BroadAccess::All
            } else if any(&this) {
                BroadAccess::Some
            } else {
                BroadAccess::None
            }
        }

        match kind {
            BroadAccessKind::ReadComponents => on(
                self,
                Access::has_any_component_read,
                Access::has_read_all_components,
            ),
            BroadAccessKind::WriteComponents => on(
                self,
                Access::has_any_component_write,
                Access::has_write_all_components,
            ),
            BroadAccessKind::ReadResources => on(
                self,
                Access::has_any_resource_read,
                Access::has_read_all_resources,
            ),
            BroadAccessKind::WriteResources => on(
                self,
                Access::has_any_resource_write,
                Access::has_write_all_resources,
            ),
        }
    }

    fn narrow(&self, cid: ComponentId) -> NarrowAccess {
        if self.has_component_write(cid) || self.has_resource_write(cid) {
            NarrowAccess::Write
        } else if self.has_component_read(cid) || self.has_resource_read(cid) {
            NarrowAccess::Read
        } else {
            NarrowAccess::None
        }
    }
}

#[track_caller]
pub fn diagnose_conflicts(components: &Components, new: AccessDiags, prev: Vec<AccessDiags>) {
    use prettytable::{row, Cell, Row, Table};
    fn mk_row(
        label: &str,
        new: &AccessDiags,
        prevs: &[AccessDiags],
        f: impl Fn(&AccessDiags) -> String,
    ) -> Row {
        let mut row = row![r->label, c->f(new)];
        for prev in prevs {
            row.add_cell(Cell::new(&f(prev)).style_spec("c"));
        }
        row
    }

    let (broad, narrow): (Vec<AccessDiags>, Vec<AccessDiags>) = prev
        .into_iter()
        .partition(|a| a.get_conflicts(&new) == AccessConflicts::All);
    let mut msg = format!(
        "\nNested system data access conflicts between {} and still-running systems:",
        new.name
    );

    if !broad.is_empty() {
        let mut table = Table::new();
        let mut titles = row![""];
        titles.add_cell(Cell::new(&BroadAccessKind::ReadComponents.to_string()).style_spec("c"));
        titles.add_cell(Cell::new(&BroadAccessKind::WriteComponents.to_string()).style_spec("c"));
        titles.add_cell(Cell::new(&BroadAccessKind::ReadResources.to_string()).style_spec("c"));
        titles.add_cell(Cell::new(&BroadAccessKind::WriteResources.to_string()).style_spec("c"));
        table.set_titles(titles);
        let row = table.add_row(row![br->&format!("~~{}~~", new.name)]);
        row.add_cell(
            Cell::new(&new.broad(BroadAccessKind::ReadComponents).to_string()).style_spec("c"),
        );
        row.add_cell(
            Cell::new(&new.broad(BroadAccessKind::WriteComponents).to_string()).style_spec("c"),
        );
        row.add_cell(
            Cell::new(&new.broad(BroadAccessKind::ReadResources).to_string()).style_spec("c"),
        );
        row.add_cell(
            Cell::new(&new.broad(BroadAccessKind::WriteResources).to_string()).style_spec("c"),
        );

        for a in broad {
            let row = table.add_row(row![r->&a.name]);
            row.add_cell(
                Cell::new(&a.broad(BroadAccessKind::ReadComponents).to_string()).style_spec("c"),
            );
            row.add_cell(
                Cell::new(&a.broad(BroadAccessKind::WriteComponents).to_string()).style_spec("c"),
            );
            row.add_cell(
                Cell::new(&a.broad(BroadAccessKind::ReadResources).to_string()).style_spec("c"),
            );
            row.add_cell(
                Cell::new(&a.broad(BroadAccessKind::WriteResources).to_string()).style_spec("c"),
            );
        }

        let _ = write!(&mut msg, "\n\n{table}");
    }

    if !narrow.is_empty() {
        let mut table = Table::new();
        let mut bits = fixedbitset::FixedBitSet::new();
        for a in &narrow {
            let AccessConflicts::Individual(bytes) = a.get_conflicts(&new) else {
                panic!("enum variant magically changed");
            };
            bits.union_with(&bytes);
        }

        let mut titles = row![""];
        let mut row = row![rb->&format!("~~{}~~", new.name)];
        for cid in bits.ones().map(ComponentId::new) {
            let name = components.get_info(cid).map_or("+++ERROR+++", |c| c.name());
            titles.add_cell(Cell::new(name).style_spec("c"));
            row.add_cell(Cell::new(&new.narrow(cid).to_string()).style_spec("cb"));
        }
        table.set_titles(titles);
        table.add_row(row);

        for a in narrow {
            let mut row = row![r->&a.name];
            for cid in bits.ones().map(ComponentId::new) {
                row.add_cell(Cell::new(&a.narrow(cid).to_string()).style_spec("c"));
            }
            table.add_row(row);
        }

        let _ = write!(&mut msg, "\n\n{table}");
    }
    error!("{}", msg);
}

/*
pub trait NestedSystemExts {
    fn run_nested<Out>(&mut self, s: NestedSystemId<(), Out>) -> Out;
    fn run_nested_with<ArgInner, Out: 'static>(
        &mut self,
        s: NestedSystemId<ArgInner, Out>,
        arg: ArgInner,
    ) -> Out;
}

impl NestedSystemExts for World {
    fn run_nested<Out: 'static>(&mut self, s: NestedSystemId<(), Out>) -> Out {
        NestedSystem::scope(self, |nested| nested.run_nested(s))
    }
    fn run_nested_with<ArgInner, Out: 'static>(
        &mut self,
        s: NestedSystemId<ArgInner, Out>,
        arg: ArgInner,
    ) -> Out {
        NestedSystem::scope(self, |nested| nested.run_nested_with(s, arg))
    }
}
    */

pub struct WithName<S> {
    sys: S,
    name: Cow<'static, str>,
}

impl<S> WithName<S> {
    pub fn new(sys: S, name: Cow<'static, str>) -> Self { Self { sys, name } }
}

pub fn with_name<S>(sys: S, name: &'static str) -> WithName<S> { WithName::new(sys, name.into()) }

#[macro_export]
macro_rules! named (
    ($sys:expr) => (named!($sys, $sys));
    ($sys:expr, $name:expr) => (crate::ecs::nested::with_name($sys, stringify!($name)));
);

impl<S, I, O, M> IntoSystem<I, O, (WithName<S>, M)> for WithName<S>
where
    S: IntoSystem<I, O, M>,
    I: SystemInput,
{
    type System = WithName<<S as IntoSystem<I, O, M>>::System>;

    fn into_system(this: Self) -> Self::System {
        WithName {
            sys: IntoSystem::into_system(this.sys),
            name: this.name,
        }
    }
}

#[rustfmt::skip]
impl<S> System for WithName<S>
where S: System
{
    type In = <S as System>::In;
    type Out = <S as System>::Out;

    // SAFETY: It's a purely forwarding implementation except for name()
    fn name(&self) -> Cow<'static, str> { self.name.clone() }
    fn component_access(&self) -> &Access<ComponentId> { self.sys.component_access()  }
    fn archetype_component_access(&self) -> &Access<ArchetypeComponentId> { self.sys.archetype_component_access()  }
    fn is_send(&self) -> bool { self.sys.is_send()  }
    fn is_exclusive(&self) -> bool { self.sys.is_exclusive()  }
    fn has_deferred(&self) -> bool { self.sys.has_deferred()  }
    unsafe fn run_unsafe(&mut self, input: SystemIn<'_, Self>, world: UnsafeWorldCell) -> Self::Out { self.sys.run_unsafe(input, world)  }
    fn apply_deferred(&mut self, world: &mut World) { self.sys.apply_deferred(world)  }
    fn queue_deferred(&mut self, world: DeferredWorld) { self.sys.queue_deferred(world)  }
    unsafe fn validate_param_unsafe(&mut self, world: UnsafeWorldCell) -> bool { self.sys.validate_param_unsafe(world)  }
    fn initialize(&mut self, world: &mut World) { self.sys.initialize(world)  }
    fn update_archetype_component_access(&mut self, world: UnsafeWorldCell) { self.sys.update_archetype_component_access(world)  }
    fn check_change_tick(&mut self, change_tick: Tick) { self.sys.check_change_tick(change_tick)  }
    fn get_last_run(&self) -> Tick { self.sys.get_last_run()  }
    fn set_last_run(&mut self, last_run: Tick) { self.sys.set_last_run(last_run)  }
    fn type_id(&self) -> TypeId { self.sys.type_id() }
    fn run(&mut self, input: SystemIn<'_, Self>, world: &mut World) -> Self::Out { self.sys.run(input, world) }
    fn validate_param(&mut self, world: &World) -> bool { self.sys.validate_param(world) }
    fn default_system_sets(&self) -> Vec<InternedSystemSet> {self.sys.default_system_sets()  }
}

#[cfg(test)]
mod test {
    use std::f32::consts::PI;

    use bevy::log::LogPlugin;

    use super::*;

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
