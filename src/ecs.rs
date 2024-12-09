use std::marker::PhantomData;

use bevy::{ecs::system::IntoObserverSystem, prelude::*};
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

pub trait HasEntityCommands<'w> {
    fn id(&self) -> Entity;
    fn insert<B: Bundle>(&mut self, bundle: B) -> &mut Self;

    fn observe<E, B, M>(&mut self, system: impl IntoObserverSystem<E, B, M>) -> &mut Self
    where
        E: Event,
        B: Bundle;

    fn commands(&mut self) -> Commands;
}

impl<'w> HasEntityCommands<'w> for EntityCommands<'w> {
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
impl<'w> HasEntityCommands<'w> for EntityWorldMut<'w> {
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

pub struct EntityCommandsOf<'a, 'w: 'a, E: HasEntityCommands<'w>, C: Component> {
    entity: &'a mut E,
    _c: Invariant<C>,
    _w: Invariant<Lifetime<'w>>,
}
impl<'a, 'w: 'a, C: Component, E: HasEntityCommands<'w>> EntityCommandsOf<'a, 'w, E, C> {
    pub fn new(entity: &'a mut E) -> Self {
        Self {
            entity,
            _c: default(),
            _w: default(),
        }
    }
}
impl<'a, 'w: 'a, C: Component, E: HasEntityCommands<'w>> From<&'a mut E>
    for EntityCommandsOf<'a, 'w, E, C>
{
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

impl<'w, C: Component, E: HasEntityCommands<'w>> EntityExtsOf<'w, C>
    for EntityCommandsOf<'_, 'w, E, C>
{
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
    type Of<'a, C: Component>
    where
        Self: 'a,
        'w: 'a;

    fn of<'a, C: Component>(&'a mut self) -> Self::Of<'a, C>
    where
        'w: 'a;
}

impl<'w> EntityExts<'w> for EntityCommands<'w> {
    type Of<'a, C: Component>
        = EntityCommandsOf<'a, 'w, EntityCommands<'w>, C>
    where
        Self: 'a,
        'w: 'a;

    fn of<'a, C: Component>(&'a mut self) -> Self::Of<'a, C>
    where
        'w: 'a,
    {
        Self::Of::from(self)
    }
}

#[track_caller]
/// Panics if id is not in the world.
pub fn run_instanced<'a, A, I, O, M, S>(world: &mut World, system: S, target: Entity, args: A) -> O
where
    A: 'a,
    I: SystemInput<Inner<'a> = (Entity, A)>,
    S: IntoSystem<I, O, M>,
{
    if !world
        .entity(target)
        .contains::<InstancedSystem<<S as IntoSystem<I, O, M>>::System>>()
    {
        let mut sys = S::into_system(system);
        sys.initialize(world);
        world.entity_mut(target).insert(InstancedSystem(Some(sys)));
    }
    let mut sys = world
        .get_mut::<InstancedSystem<<S as IntoSystem<I, O, M>>::System>>(target)
        .unwrap()
        .take()
        .unwrap_or_else(|| panic!("System is reentrant"));
    let out = sys.run((target, args), world);
    sys.apply_deferred(world);
    world.entity_mut(target).insert(InstancedSystem(Some(sys)));
    out
}

#[derive(Component, Deref, DerefMut, Resource, Debug, Copy, Clone)]
pub struct InstancedSystem<S: System>(Option<S>);
