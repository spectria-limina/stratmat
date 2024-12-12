use std::ops::{Deref, DerefMut};

use bevy::ecs::{
    query::{QueryData, QueryItem, ROQueryItem},
    system::{ParamBuilder, SystemParam},
};
pub use bevy::prelude::*;

use super::*;

pub struct Given<'w, 's, D: QueryData, Marker = ()> {
    given: Entity,
    query: Query<'w, 's, D>,
    _ph: PhantomData<Marker>,
}

impl<'w, 's, D: QueryData, Label> Given<'w, 's, D, Label> {
    pub fn new(given: Entity, query: Query<'w, 's, D>) -> Self {
        Self {
            given,
            query,
            _ph: PhantomData,
        }
    }

    pub fn get(&self) -> ROQueryItem<D> { self.query.get(self.given).unwrap() }
    pub fn get_mut(&mut self) -> QueryItem<D> { self.query.get_mut(self.given).unwrap() }
}

impl<'w, 's, D: QueryData, Label> Deref for Given<'w, 's, D, Label> {
    type Target = D;

    fn deref(&self) -> &Self::Target { todo!() }
}
impl<'w, 's, D: QueryData, Label> DerefMut for Given<'w, 's, D, Label> {
    fn deref_mut(&mut self) -> &mut Self::Target { todo!() }
}

unsafe impl<'w, 's, D: QueryData + 'static, Label> SystemParam for Given<'w, 's, D, Label> {
    type State = (Entity, <Query<'w, 's, D> as SystemParam>::State);
    type Item<'world, 'state> = Given<'world, 'state, D, Label>;

    fn init_state(
        _world: &mut World,
        _system_meta: &mut bevy::ecs::system::SystemMeta,
    ) -> Self::State {
        panic!("Given must be initialized by a SystemParamBuilder to provide an Entity");
    }

    unsafe fn get_param<'world, 'state>(
        &mut (given, ref mut query_state): &'state mut Self::State,
        _system_meta: &bevy::ecs::system::SystemMeta,
        _world: UnsafeWorldCell<'world>,
        _change_tick: Tick,
    ) -> Self::Item<'world, 'state> {
        // SAFETY: The state was initialized using the GivenParamBuilder, which forwards to Query.
        unsafe {
            Self::Item::new(
                given,
                <Query<'w, 's, D> as SystemParam>::get_param(
                    query_state,
                    _system_meta,
                    _world,
                    _change_tick,
                ),
            )
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct GivenBuilder {
    given: Entity,
}

impl GivenBuilder {
    pub fn new(given: Entity) -> Self { Self { given } }
}

unsafe impl<'w, 's, D: QueryData + 'static, Label> SystemParamBuilder<Given<'w, 's, D, Label>>
    for GivenBuilder
{
    fn build(
        self,
        world: &mut World,
        meta: &mut bevy::ecs::system::SystemMeta,
    ) -> <Given<'w, 's, D, Label> as SystemParam>::State {
        (
            self.given,
            ParamBuilder::of::<Query<D>>().build(world, meta),
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[cfg(test)]
    fn no_val_panic() {
        let mut world = World::new();

        let mut sys = (ParamBuilder,)
            .build_state(&mut world)
            .build_any_system(|given: Given<Entity>| given.get());

        let _result: Entity = sys.run((), &mut world);
    }

    #[cfg(test)]
    fn entity() {
        let mut world = World::new();

        let entity = world.spawn(()).id();
        let mut sys = (GivenBuilder::new(entity),)
            .build_state(&mut world)
            .build_any_system(|given: Given<Entity>| given.get());

        let result: Entity = sys.run((), &mut world);

        assert_eq!(result, entity);
    }

    #[cfg(test)]
    fn component() {
        let mut world = World::new();

        #[derive(Component)]
        struct C(u32);

        let entity = world.spawn_batch((1..10).map(C)).nth(7).unwrap();
        let mut sys = (GivenBuilder::new(entity),)
            .build_state(&mut world)
            .build_any_system(|given: Given<&C>| given.get().0);

        let result: u32 = sys.run((), &mut world);

        assert_eq!(result, 32);
    }
}
