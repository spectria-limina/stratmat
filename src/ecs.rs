use std::{any::Any, borrow::Cow, marker::PhantomData};

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
    _ph: PhantomData<(&'w mut (), C)>,
}
impl<'a, 'w: 'a, C: Component, E: EntityScope<'w>> ScopedOn<'a, 'w, E, C> {
    pub fn new(entity: &'a mut E) -> Self {
        Self {
            entity,
            _ph: default(),
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

pub type NestedSystemInput<'a, Data, Arg> = (NestedSystems<'a>, Data, Arg);

impl<'a> SystemInput for NestedSystems<'a> {
    type Param<'i>
        = NestedSystems<'i>
    where
        Self: 'i;
    type Inner<'i>
        = NestedSystemInput<'i, (), ()>
    where
        Self: 'i;

    fn wrap<'i>((ns, _, _): Self::Inner<'i>) -> Self::Param<'i>
    where
        Self: 'i,
    {
        ns
    }
}
pub struct NestedWithArg<'a, Arg>(NestedSystems<'a>, Arg);

impl<'a, Arg> SystemInput for NestedWithArg<'a, Arg> {
    type Param<'i>
        = NestedWithArg<'i, Arg>
    where
        Self: 'i;
    type Inner<'i>
        = NestedSystemInput<'i, (), Arg>
    where
        Self: 'i;

    fn wrap<'i>((ns, _, arg): Self::Inner<'i>) -> Self::Param<'i>
    where
        Self: 'i,
    {
        NestedWithArg(ns, arg)
    }
}

pub struct NestedWithData<'a, Data>(NestedSystems<'a>, Data);

impl<'a, Data> SystemInput for NestedWithData<'a, Data> {
    type Param<'i>
        = NestedWithData<'i, Data>
    where
        Self: 'i;
    type Inner<'i>
        = NestedSystemInput<'i, Data, ()>
    where
        Self: 'i;

    fn wrap<'i>((ns, data, _): Self::Inner<'i>) -> Self::Param<'i>
    where
        Self: 'i,
    {
        NestedWithData(ns, data)
    }
}

pub struct NestedWith<'a, Data, Arg>(NestedSystems<'a>, Data, Arg);

impl<'a, Data, Arg> SystemInput for NestedWith<'a, Data, Arg> {
    type Param<'i>
        = NestedWith<'i, Data, Arg>
    where
        Self: 'i;
    type Inner<'i>
        = NestedSystemInput<'i, Data, Arg>
    where
        Self: 'i;

    fn wrap<'i>((ns, data, arg): Self::Inner<'i>) -> Self::Param<'i>
    where
        Self: 'i,
    {
        NestedWith(ns, data, arg)
    }
}

pub trait ErasedNestedSystem: Send + Sync {
    fn queue_deferred(&mut self, world: DeferredWorld);
    fn name(&self) -> Cow<'static, str>;
    fn update_archetype_component_access(&mut self, world: UnsafeWorldCell<'_>);
    fn archetype_component_access(&self) -> &Access<ArchetypeComponentId>;
    // arg MUST be the Arg type.
    // the return type is always the Out type.
    unsafe fn run<'w>(&mut self, nested: NestedSystems<'w>, arg: OwningPtr<'_>) -> Box<dyn Any>;
}

impl<Sys, Data, Arg> ErasedNestedSystem for NestedSystem<Sys, Data>
where
    Sys: System,
    <Sys as System>::In: for<'a> SystemInput<Inner<'a> = NestedSystemInput<'a, Data, Arg>>,
    Data: Clone + Send + Sync,
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

    unsafe fn run<'w>(&mut self, nested: NestedSystems<'w>, arg: OwningPtr<'_>) -> Box<dyn Any> {
        // FIXME: Validate argument type?!

        let world = nested.world;
        let input: SystemIn<Sys> = (nested, self.data.clone(), arg.read());
        let out = unsafe { self.sys.run_unsafe(input, world) };
        unsafe {
            self.sys.queue_deferred(world.into_deferred());
        }
        Box::new(out)
    }
}

type CachedSystem = Cached<Box<dyn ErasedNestedSystem>>;

#[derive(Resource, Default)]
pub struct NestedSystemRegistry {
    store: Vec<CachedSystem>,
}

impl NestedSystemRegistry {
    pub fn new() -> Self {
        default()
    }

    #[allow(private_bounds)]
    pub fn register<Sys, In, Arg, Out, Marker>(
        world: &mut World,
        s: Sys,
    ) -> NestedSystemId<Arg, Out>
    where
        Sys: IntoSystem<In, Out, Marker>,
        In: for<'a> SystemInput<Inner<'a> = NestedSystemInput<'a, (), Arg>> + 'static,
        Arg: 'static,
        Out: 'static,
    {
        Self::register_with_data(world, s, ())
    }

    #[allow(clippy::type_complexity)]
    // FIXME
    #[allow(private_bounds)]
    pub fn register_with_data<Sys, In, Data, Arg, Out, Marker>(
        world: &mut World,
        s: Sys,
        data: Data,
    ) -> NestedSystemId<Arg, Out>
    where
        Sys: IntoSystem<In, Out, Marker>,
        In: for<'a> SystemInput<Inner<'a> = NestedSystemInput<'a, Data, Arg>> + 'static,
        Data: Clone + Send + Sync + 'static,
        Arg: 'static,
        Out: 'static,
    {
        let mut sys = IntoSystem::into_system(s);
        sys.initialize(world);
        let mut registry = world.resource_mut::<NestedSystemRegistry>();
        registry
            .store
            .push(Cached::new(Box::new(NestedSystem::new(sys, data))));
        NestedSystemId(registry.store.len() - 1, PhantomData)
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, derive_more::Debug)]
pub struct NestedSystemId<Arg: 'static = (), Out: 'static = ()>(
    usize,
    #[debug(skip)] PhantomData<(Arg, Out)>,
);

pub struct NestedSystems<'w> {
    accesses: &'w mut Vec<(String, Access<ArchetypeComponentId>)>,
    world: UnsafeWorldCell<'w>,
    registry: &'w mut NestedSystemRegistry,
}

impl NestedSystems<'_> {
    pub fn scope<'world, R, F: for<'w> FnOnce(NestedSystems<'w>) -> R>(
        world: &'world mut World,
        f: F,
    ) {
        world.resource_scope(
            |world: &mut World, mut registry: Mut<NestedSystemRegistry>| {
                let mut accesses = vec![];
                let mut this = NestedSystems {
                    accesses: &mut accesses,
                    world: world.as_unsafe_world_cell(),
                    registry: &mut registry,
                };
                f(this)
            },
        );
    }

    pub fn run_nested<Out: 'static>(self, s: NestedSystemId<(), Out>) -> Out {
        self.run_nested_with(s, ())
    }

    pub fn run_nested_with<Arg, Out: 'static>(self, s: NestedSystemId<Arg, Out>, arg: Arg) -> Out {
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
        // SAFETY: The NestedSystemId tells us that arg is the correct type.
        //         Even if the caller launders it between registries they can't change the type.
        let world = self.world;
        let out = OwningPtr::make(arg, |ptr| unsafe { sys.run(self, ptr) });
        // SAFETY: The only thing we're touching is the command queue,
        //         we never let any other caller touch that.
        unsafe {
            sys.queue_deferred(world.into_deferred());
        }
        // FIXME: Do we need to poison/abort if a panic comes through here? Figure that out.
        // self.accesses.pop();
        match out.downcast::<Out>() {
            Ok(out) => *out,
            Err(_) => panic!(
                "Nested system {:?} gave us the wrong output. Expected {}. Yikes!",
                s,
                std::any::type_name::<Out>()
            ),
        }
    }
}

fn shorten<'a, 'b: 'a, T>(t: &'b mut T) -> &'a mut T {
    t
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
        NestedWith(ns, data, (arg, id_r_c1_again, id_rw_c2_c3, id_w_c1)): NestedWith<
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
    fn r_c1_again(
        NestedWithArg(ns, (id_rw_c2_c3, id_w_c1)): NestedWithArg<(IdRwC2C3, IdWC1)>,
        _q: Query<&C1>,
    ) {
        info!("r_c1_again");
        ns.run_nested_with(id_rw_c2_c3, id_w_c1);
    }
    fn rw_c2_c3(NestedWithArg(ns, id_w_c1): NestedWithArg<IdWC1>, _q: Query<(&C2, &mut C3)>) {
        info!("rw_c2_c3");
        ns.run_nested(id_w_c1);
    }
    fn w_c1(_ns: NestedSystems, _q: Query<&mut C1>) {
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
