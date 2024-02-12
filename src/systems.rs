use bevy::{ecs::system::SystemId, prelude::*, utils::HashMap};
use bevy_commandify::command;

#[derive(Resource, Clone, Default, Debug)]
pub struct Registry(HashMap<String, SystemId>);

pub trait RegistryExt {
    fn register<M, S: IntoSystem<(), (), M>>(&mut self, system: S) -> &mut Self;
    fn run<M, S: IntoSystem<(), (), M>>(&mut self, system: S);
}

impl RegistryExt for World {
    fn register<M, S: IntoSystem<(), (), M>>(&mut self, system: S) -> &mut Self {
        self.init_resource::<Registry>();
        self.resource_scope::<Registry, _>(|world, mut registry| {
            let system = S::into_system(system);
            let name = system.name().to_string();
            let system_id = world.register_system(system);
            registry
                .0
                .try_insert(name.clone(), system_id)
                .expect(&format!("Duplicate registration of system {name}"));
            info!("Registered system {name}")
        });
        self
    }

    fn run<M, S: IntoSystem<(), (), M>>(&mut self, system: S) {
        self.resource_scope::<Registry, _>(|world, registry| {
            let system = S::into_system(system);
            let id = registry
                .0
                .get(&*system.name())
                .expect(&format!("System {} not registered", system.name()));
            world.run_system(*id).unwrap();
        });
    }
}

impl RegistryExt for App {
    fn register<M, S: IntoSystem<(), (), M>>(&mut self, system: S) -> &mut Self {
        self.world.register(system);
        self
    }

    fn run<M, S: IntoSystem<(), (), M>>(&mut self, system: S) {
        self.world.run(system);
    }
}

#[command]
fn register_impl<S: System<In = (), Out = ()>>(world: &mut World, system: S) {
    world.register(system);
}
#[command]
fn run_impl<S: System<In = (), Out = ()>>(world: &mut World, system: S) {
    world.run(system);
}

impl RegistryExt for Commands<'_, '_> {
    fn register<M, S: IntoSystem<(), (), M>>(&mut self, system: S) -> &mut Self {
        self.register_impl(S::into_system(system));
        self
    }

    fn run<M, S: IntoSystem<(), (), M>>(&mut self, system: S) {
        self.run_impl(S::into_system(system))
    }
}
