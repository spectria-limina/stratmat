use std::{borrow::Cow, marker::PhantomData};

use bevy::{
    ecs::{
        archetype::ArchetypeComponentId,
        query::Access,
        system::IntoObserverSystem,
        world::{unsafe_world_cell::UnsafeWorldCell, DeferredWorld},
    },
    prelude::*,
    ptr::OwningPtr,
};
use type_variance::{Invariant, Lifetime};

/// Marker component for child entities added by a specific component.
#[derive(Component, Copy, Clone, Default, Debug)]
pub struct ChildFor<C: Component>(PhantomData<C>);

impl<C: Component> ChildFor<C> {
    pub fn new() -> Self {
        ChildFor(PhantomData)
    }

    pub fn despawn_for(
        In(id): In<Entity>,
        q: Query<&Children, With<Self>>,
        mut commands: Commands,
    ) {
        for observer in q.iter_descendants(id) {
            commands.entity(observer).despawn();
        }
    }
}

pub trait EntityScope<'w> {
    fn id(&self) -> Entity;
    fn insert<B: Bundle>(&mut self, bundle: B) -> &mut Self;

    fn observe<E, B, M>(&mut self, system: impl IntoObserverSystem<E, B, M>) -> &mut Self
    where
        E: Event,
        B: Bundle;

    fn commands(&mut self) -> Commands;
}

impl<'w> EntityScope<'w> for EntityCommands<'w> {
    fn id(&self) -> Entity {
        self.id()
    }

    fn insert<B: Bundle>(&mut self, bundle: B) -> &mut Self {
        self.insert(bundle)
    }

    fn observe<E, B, M>(&mut self, system: impl IntoObserverSystem<E, B, M>) -> &mut Self
    where
        E: Event,
        B: Bundle,
    {
        self.observe(system)
    }

    fn commands(&mut self) -> Commands {
        self.commands()
    }
}
impl<'w> EntityScope<'w> for EntityWorldMut<'w> {
    fn id(&self) -> Entity {
        self.id()
    }

    fn insert<B: Bundle>(&mut self, bundle: B) -> &mut Self {
        self.insert(bundle)
    }

    fn observe<E, B, M>(&mut self, system: impl IntoObserverSystem<E, B, M>) -> &mut Self
    where
        E: Event,
        B: Bundle,
    {
        self.observe(system)
    }

    fn commands(&mut self) -> Commands {
        // SAFETY: A commands object doesn't let you directly mutate entity storage.
        unsafe { self.world_mut().commands() }
    }
}

pub struct ScopedOn<'a, 'w: 'a, E: EntityScope<'w>, C: Component> {
    entity: &'a mut E,
    _c: Invariant<C>,
    _w: Invariant<Lifetime<'w>>,
}
impl<'a, 'w: 'a, C: Component, E: EntityScope<'w>> ScopedOn<'a, 'w, E, C> {
    pub fn new(entity: &'a mut E) -> Self {
        Self {
            entity,
            _c: default(),
            _w: default(),
        }
    }
}
impl<'a, 'w: 'a, C: Component, E: EntityScope<'w>> From<&'a mut E> for ScopedOn<'a, 'w, E, C> {
    fn from(entity: &'a mut E) -> Self {
        Self::new(entity)
    }
}

pub trait EntityExtsOf<'w, C: Component> {
    type Unscoped;

    fn observe<V, B, M>(&mut self, system: impl IntoObserverSystem<V, B, M>) -> &mut Self::Unscoped
    where
        V: Event,
        B: Bundle;

    fn despawn_children(&mut self) -> &mut Self;
}

impl<'w, C: Component, E: EntityScope<'w>> EntityExtsOf<'w, C> for ScopedOn<'_, 'w, E, C> {
    type Unscoped = E;

    fn observe<V, B, M>(&mut self, system: impl IntoObserverSystem<V, B, M>) -> &mut Self::Unscoped
    where
        V: Event,
        B: Bundle,
    {
        self.entity.observe(system).insert(ChildFor::<C>::new())
    }

    fn despawn_children(&mut self) -> &mut Self {
        let id = self.entity.id();
        self.entity
            .commands()
            .run_system_cached_with(ChildFor::<C>::despawn_for, id);
        self
    }
}

pub trait EntityExts<'w> {
    type On<'a, C: Component>
    where
        Self: 'a,
        'w: 'a;

    fn on<'a, C: Component>(&'a mut self) -> Self::On<'a, C>
    where
        'w: 'a;
}

impl<'w> EntityExts<'w> for EntityCommands<'w> {
    type On<'a, C: Component>
        = ScopedOn<'a, 'w, EntityCommands<'w>, C>
    where
        Self: 'a,
        'w: 'a;

    fn on<'a, C: Component>(&'a mut self) -> Self::On<'a, C>
    where
        'w: 'a,
    {
        Self::On::from(self)
    }
}

impl<'w> EntityExts<'w> for EntityWorldMut<'w> {
    type On<'a, C: Component>
        = ScopedOn<'a, 'w, EntityWorldMut<'w>, C>
    where
        Self: 'a,
        'w: 'a;

    fn on<'a, C: Component>(&'a mut self) -> Self::On<'a, C>
    where
        'w: 'a,
    {
        Self::On::from(self)
    }
}

pub trait EntityWorldExts<'w> {
    fn run_instanced<'a, I, O, M, S>(&mut self, system: S) -> O
    where
        I: SystemInput<Inner<'a> = (Entity, ())> + 'a,
        S: IntoSystem<I, O, M>;

    fn run_instanced_with<'a, A, I, O, M, S>(&mut self, system: S, args: A) -> O
    where
        A: 'a,
        I: SystemInput<Inner<'a> = (Entity, A)> + 'a,
        S: IntoSystem<I, O, M>;
}

impl<'w> EntityWorldExts<'w> for EntityWorldMut<'w> {
    #[track_caller]
    /// Panics if id is not in the world.
    fn run_instanced_with<'a, A, I, O, M, S>(&mut self, system: S, args: A) -> O
    where
        A: 'a,
        I: SystemInput<Inner<'a> = (Entity, A)> + 'a,
        S: IntoSystem<I, O, M>,
    {
        let target = self.id();
        self.world_scope(move |world: &mut World| {
            if !world
                .entity(target)
                .contains::<Cached<<S as IntoSystem<I, O, M>>::System>>()
            {
                let mut sys = S::into_system(system);
                sys.initialize(world);
                world.entity_mut(target).insert(Cached::new(sys));
            }
            let mut sys = world
                .get_mut::<Cached<<S as IntoSystem<I, O, M>>::System>>(target)
                .unwrap()
                .take()
                .unwrap_or_else(|| panic!("System is reentrant"));
            let out = sys.run((target, args), world);
            sys.apply_deferred(world);
            world.entity_mut(target).insert(Cached::new(sys));
            out
        })
    }
    #[track_caller]
    /// Panics if id is not in the world.
    fn run_instanced<'a, I, O, M, S>(&mut self, system: S) -> O
    where
        I: SystemInput<Inner<'a> = (Entity, ())> + 'a,
        S: IntoSystem<I, O, M>,
    {
        self.run_instanced_with(system, ())
    }
}

#[derive(Component, Resource, Debug, Copy, Clone)]
pub enum Cached<S> {
    Stored(S),
    InUse,
}

impl<S> Cached<S> {
    pub fn new(s: S) -> Self {
        Self::Stored(s)
    }

    pub fn take(&mut self) -> Option<S> {
        let mut swap = Self::InUse;
        std::mem::swap(&mut swap, self);
        swap.into()
    }
}

impl<S> From<Cached<S>> for Option<S> {
    fn from(value: Cached<S>) -> Self {
        match value {
            Cached::Stored(s) => Some(s),
            Cached::InUse => None,
        }
    }
}

struct NestedSystem<Sys, Data> {
    sys: Sys,
    data: Data,
}

impl<Sys, Data> NestedSystem<Sys, Data> {
    fn new(s: Sys, d: Data) -> Self {
        Self { sys: s, data: d }
    }
}

pub trait ErasedNestedSystem<'w>: Send + Sync {
    fn queue_deferred(&mut self, world: DeferredWorld);
    fn name(&self) -> Cow<'static, str>;
    fn update_archetype_component_access(&mut self, world: UnsafeWorldCell<'_>);
    fn archetype_component_access(&self) -> &Access<ArchetypeComponentId>;
    // arg MUST be the Arg type.
    // the return type is always the Out type.
    unsafe fn run<'a>(&'a mut self, nested: &'a mut NestedSystems<'w>, arg: OwningPtr<'a>);
}

pub type NestedSystemInput<'a, 'w, Data, Arg> = (&'a mut NestedSystems<'w>, Data, Arg);

impl<'a, 'w: 'a> SystemInput for &'a mut NestedSystems<'w> {
    type Param<'i>
        = &'i mut NestedSystems<'w>
    where
        Self: 'i;
    type Inner<'i>
        = NestedSystemInput<'i, 'w, (), ()>
    where
        Self: 'i;

    fn wrap<'i>((ns, _, _): Self::Inner<'i>) -> Self::Param<'i>
    where
        Self: 'i,
    {
        ns
    }
}
pub struct WithArg<'a, 'w, Arg>(&'a mut NestedSystems<'w>, Arg);

impl<'a, 'w: 'a, Arg> SystemInput for WithArg<'a, 'w, Arg> {
    type Param<'i>
        = WithArg<'i, 'w, Arg>
    where
        Self: 'i;
    type Inner<'i>
        = NestedSystemInput<'i, 'w, (), Arg>
    where
        Self: 'i;

    fn wrap<'i>((ns, _, arg): Self::Inner<'i>) -> Self::Param<'i>
    where
        Self: 'i,
    {
        WithArg(ns, arg)
    }
}

pub struct WithData<'a, 'w, Data>(&'a mut NestedSystems<'w>, Data);

impl<'a, 'w: 'a, Data> SystemInput for WithData<'a, 'w, Data> {
    type Param<'i>
        = WithData<'i, 'w, Data>
    where
        Self: 'i;
    type Inner<'i>
        = NestedSystemInput<'i, 'w, Data, ()>
    where
        Self: 'i;

    fn wrap<'i>((ns, data, _): Self::Inner<'i>) -> Self::Param<'i>
    where
        Self: 'i,
    {
        WithData(ns, data)
    }
}

pub struct Nested<'a, 'w, Data, Arg>(&'a mut NestedSystems<'w>, Data, Arg);

impl<'a, 'w: 'a, Data, Arg> SystemInput for Nested<'a, 'w, Data, Arg> {
    type Param<'i>
        = Nested<'i, 'w, Data, Arg>
    where
        Self: 'i;
    type Inner<'i>
        = NestedSystemInput<'i, 'w, Data, Arg>
    where
        Self: 'i;

    fn wrap<'i>((ns, data, arg): Self::Inner<'i>) -> Self::Param<'i>
    where
        Self: 'i,
    {
        Nested(ns, data, arg)
    }
}

impl<'w, Sys, Data, Arg> ErasedNestedSystem<'w> for NestedSystem<Sys, Data>
where
    Sys: System,
    <Sys as System>::In: for<'a> SystemInput<Inner<'a> = NestedSystemInput<'a, 'w, Data, Arg>>,
    Data: Clone + Send + Sync + 'static,
    Arg: 'static,
    <Sys as System>::Out: 'static,
{
    fn name(&self) -> Cow<'static, str> {
        self.sys.name()
    }
    fn update_archetype_component_access(&mut self, world: UnsafeWorldCell<'_>) {
        self.sys.update_archetype_component_access(world);
    }
    fn queue_deferred(&mut self, world: DeferredWorld) {
        self.sys.queue_deferred(world);
    }
    fn archetype_component_access(&self) -> &Access<ArchetypeComponentId> {
        self.sys.archetype_component_access()
    }

    unsafe fn run<'a>(&'a mut self, nested: &'a mut NestedSystems<'w>, arg: OwningPtr<'a>) {
        // TODO: Validate argument type?!
        let world = nested.world;

        let input: <<Sys as System>::In as SystemInput>::Inner<'a> =
            (nested, self.data.clone(), arg.read());
        unsafe {
            self.sys.run_unsafe(input, world);
            self.sys.queue_deferred(world.into_deferred());
        }
    }
}

type CachedSystem = Cached<Box<dyn for<'w> ErasedNestedSystem<'w>>>;

#[derive(Resource, Default)]
pub struct NestedSystemRegistry {
    store: Vec<CachedSystem>,
}

impl NestedSystemRegistry {
    pub fn new() -> Self {
        default()
    }

    #[allow(private_bounds)]
    #[track_caller]
    pub fn register<Sys, In, Arg, Out, Marker>(
        world: &mut World,
        s: Sys,
    ) -> NestedSystemId<Arg, Out>
    where
        Sys: IntoSystem<In, Out, Marker>,
        In: for<'a> SystemInput<Inner<'a> = NestedSystemInput<'a, 'static, (), Arg>> + 'static,
        Arg: 'static,
        Out: 'static,
        NestedSystem<<Sys as bevy::prelude::IntoSystem<In, Out, Marker>>::System, ()>:
            for<'w> ErasedNestedSystem<'w>,
    {
        Self::register_with_data(world, s, ())
    }

    #[allow(clippy::type_complexity)]
    // FIXME
    #[allow(private_bounds)]
    #[track_caller]
    pub fn register_with_data<Sys, In, Data, Arg, Out, Marker>(
        world: &mut World,
        s: Sys,
        data: Data,
    ) -> NestedSystemId<Arg, Out>
    where
        Sys: IntoSystem<In, Out, Marker>,
        In: for<'a> SystemInput<Inner<'a> = NestedSystemInput<'a, 'static, Data, Arg>> + 'static,
        Data: Clone + Send + Sync + 'static,
        Arg: 'static,
        Out: 'static,
        NestedSystem<<Sys as bevy::prelude::IntoSystem<In, Out, Marker>>::System, Data>:
            for<'w> ErasedNestedSystem<'w>,
    {
        let mut sys = IntoSystem::into_system(s);
        sys.initialize(world);
        let mut registry = world.resource_mut::<NestedSystemRegistry>();
        registry
            .store
            .push(Cached::new(Box::new(NestedSystem::new(sys, data))));
        NestedSystemId(registry.store.len() - 1, default(), default())
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct NestedSystemId<Arg = (), Out = ()>(usize, Invariant<Arg>, Invariant<Out>);

pub struct NestedSystems<'w> {
    accesses: Vec<(String, Access<ArchetypeComponentId>)>,
    world: UnsafeWorldCell<'w>,
    registry: &'w mut NestedSystemRegistry,
}

impl NestedSystems<'_> {
    pub fn scope<'world, R, F: for<'call, 'cell> FnOnce(&'call mut NestedSystems<'cell>) -> R>(
        world: &'world mut World,
        f: F,
    ) {
        world.resource_scope(
            |world: &mut World, mut registry: Mut<NestedSystemRegistry>| {
                let mut this = NestedSystems {
                    accesses: vec![],
                    world: world.as_unsafe_world_cell(),
                    registry: &mut registry,
                };
                f(&mut this)
            },
        );
    }

    #[track_caller]
    pub fn run_nested<Out>(&mut self, s: NestedSystemId<(), Out>) {
        self.run_nested_with(s, ())
    }

    #[track_caller]
    pub fn run_nested_with<Arg, Out>(&mut self, s: NestedSystemId<Arg, Out>, arg: Arg) {
        let Some(mut sys) = self
            .registry
            .store
            .get_mut(s.0)
            .unwrap_or_else(|| panic!("Invalid NestedSystemId {}", s.0))
            .take()
        else {
            panic!("NestedSystemId {} is (indirectly?) calling itself", s.0);
        };

        sys.update_archetype_component_access(self.world);
        let access = sys.archetype_component_access();
        dbg!(
            "Trying to run nested system {} with archetype_component_access {access:#?}",
            sys.name()
        );
        for (prev_name, prev_access) in self.accesses.iter() {
            if !prev_access.is_compatible(access) {
                panic!("Nested system {} cannot run because it conflicts with previous accesses by {prev_name} on {:?}", sys.name(), prev_access.get_conflicts(access));
            }
        }
        self.accesses.push((sys.name().to_string(), access.clone()));
        // SAFETY: We've checked that our accesses work out, added this access to the list,
        //         and we have exclusive ownership of the world.
        OwningPtr::make(arg, move |ptr| {
            unsafe {
                sys.run(self, ptr);
                sys.queue_deferred(self.world.into_deferred());
            }
            // FIXME: Do we need to poison/abort if a panic comes through here? Figure that out.
            self.accesses.pop();
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ecs::NestedSystems;

    #[derive(Component, Default, Debug)]
    struct C1;

    #[derive(Component, Default, Debug)]
    struct C2;

    #[derive(Component, Default, Debug)]
    struct C3;

    const ARG: u32 = 1728;
    const DATA: f32 = 3.9;

    type IdRC1Again = NestedSystemId<(IdRwC2C3, IdWC1)>;
    type IdRwC2C3 = NestedSystemId<IdWC1>;
    type IdWC1 = NestedSystemId;

    fn r_c1(
        Nested(ns, data, (arg, id_r_c1_again, id_rw_c2_c3, id_w_c1)): Nested<
            f32,
            (u32, IdRC1Again, IdRwC2C3, IdWC1),
        >,
        _q: Query<&C1>,
    ) {
        info!("r_c1");
        assert_eq!(arg, ARG);
        assert_eq!(data, DATA);
        ns.run_nested_with(id_r_c1_again, (id_rw_c2_c3, id_w_c1));
    }
    fn r_c1_again(WithArg(ns, (id_rw_c2_c3, id_w_c1)): WithArg<(IdRwC2C3, IdWC1)>, _q: Query<&C1>) {
        info!("r_c1_again");
        ns.run_nested_with(id_rw_c2_c3, id_w_c1);
    }
    fn rw_c2_c3(WithArg(ns, id_w_c1): WithArg<IdWC1>, _q: Query<(&C2, &mut C3)>) {
        info!("rw_c2_c3");
        ns.run_nested(id_w_c1);
    }
    fn w_c1(_ns: &mut NestedSystems, _q: Query<&mut C1>) {
        error!("w_c1... undefined behaviour... 😔");
    }

    #[test]
    fn test_nested_system_basic() {
        let mut app = App::new();
        app.init_resource::<NestedSystemRegistry>();
        let id_r_c1 = NestedSystemRegistry::register_with_data(app.world_mut(), r_c1, DATA);
        let id_r_c1_again = NestedSystemRegistry::register(app.world_mut(), r_c1_again);
        let id_rw_c2_c3 = NestedSystemRegistry::register(app.world_mut(), rw_c2_c3);
        let id_w_c1 = NestedSystemRegistry::register(app.world_mut(), w_c1);
        app.world_mut().spawn((C1, C2, C3));

        NestedSystems::scope(app.world_mut(), |ns| {
            ns.run_nested_with(id_r_c1, (ARG, id_r_c1_again, id_rw_c2_c3, id_w_c1));
        })
    }
}
