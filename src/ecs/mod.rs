use std::{self, marker::PhantomData};

use bevy::{ecs::system::IntoObserverSystem, prelude::*};

pub mod nested;

/// Marker component for child entities added by a specific component.
#[derive(Component, Copy, Clone, Default, Debug)]
pub struct ChildFor<C: Component>(PhantomData<C>);

impl<C: Component> ChildFor<C> {
    pub fn new() -> Self { ChildFor(PhantomData) }

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
    fn id(&self) -> Entity { self.id() }

    fn insert<B: Bundle>(&mut self, bundle: B) -> &mut Self { self.insert(bundle) }

    fn observe<E, B, M>(&mut self, system: impl IntoObserverSystem<E, B, M>) -> &mut Self
    where
        E: Event,
        B: Bundle,
    {
        self.observe(system)
    }

    fn commands(&mut self) -> Commands { self.commands() }
}
impl<'w> EntityScope<'w> for EntityWorldMut<'w> {
    fn id(&self) -> Entity { self.id() }

    fn insert<B: Bundle>(&mut self, bundle: B) -> &mut Self { self.insert(bundle) }

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
    fn from(entity: &'a mut E) -> Self { Self::new(entity) }
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
    pub fn new(s: S) -> Self { Self::Stored(s) }

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
