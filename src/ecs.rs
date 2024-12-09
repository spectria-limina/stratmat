use std::marker::PhantomData;

use bevy::{ecs::system::IntoObserverSystem, prelude::*};

/// Marker component for observer hooks added by a specific component.
#[derive(Component, Copy, Clone, Default, Debug)]
pub struct ObserverFor<C: Component>(PhantomData<C>);

impl<C: Component> ObserverFor<C> {
    pub fn new() -> Self {
        ObserverFor(PhantomData)
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

pub struct EntityCommandsOf<'a, 'w: 'a, C: Component> {
    entity: &'a mut EntityCommands<'w>,
    _ph: PhantomData<C>,
}
impl<'a, 'w: 'a, C: Component> EntityCommandsOf<'a, 'w, C> {
    pub fn new(entity: &'a mut EntityCommands<'w>) -> Self {
        Self {
            entity,
            _ph: PhantomData,
        }
    }
}
impl<'a, 'w: 'a, C: Component> From<&'a mut EntityCommands<'w>> for EntityCommandsOf<'a, 'w, C> {
    fn from(entity: &'a mut EntityCommands<'w>) -> Self {
        Self::new(entity)
    }
}

pub trait EntityExtsOf<'w, C: Component> {
    fn observe<E, B, M>(
        &mut self,
        system: impl IntoObserverSystem<E, B, M>,
    ) -> &mut EntityCommands<'w>
    where
        E: Event,
        B: Bundle;

    fn despawn_observers(&mut self) -> &mut Self;
}

impl<'w, C: Component> EntityExtsOf<'w, C> for EntityCommandsOf<'_, 'w, C> {
    fn observe<E, B, M>(
        &mut self,
        system: impl IntoObserverSystem<E, B, M>,
    ) -> &mut EntityCommands<'w>
    where
        E: Event,
        B: Bundle,
    {
        self.entity.observe(system)
    }

    fn despawn_observers(&mut self) -> &mut Self {
        let id = self.entity.id();
        self.entity
            .commands()
            .run_system_cached_with(ObserverFor::<C>::despawn_for, id);
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
        = EntityCommandsOf<'a, 'w, C>
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
