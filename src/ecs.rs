use std::marker::PhantomData;

use bevy::{
    app::MainScheduleOrder,
    asset::AssetEvents,
    ecs::{
        schedule::ScheduleLabel,
        system::{Command, CommandQueue, SystemId},
    },
    prelude::*,
    utils::HashMap,
};
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
                .unwrap_or_else(|e| panic!("Duplicate registration of system {name}: {e}"));
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
                .unwrap_or_else(|| panic!("System {} not registered", system.name()));
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
#[command(struct_name = Run)]
pub fn run_impl<S: System<In = (), Out = ()>>(world: &mut World, system: S) {
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

// FIXME: Remove once Commands.append exists in Bevy 0.13
#[command]
fn append(world: &mut World, queue: CommandQueue) {
    // FIXME: Work around commandify not handling mut parameters correctly.
    let mut queue = queue;
    queue.apply(world);
}

/// Resource that holds commands deferred until an asset is loaded.
#[derive(Resource)]
pub struct AssetCommands<A: Asset> {
    on_load: HashMap<AssetId<A>, CommandQueue>,
}

impl<A: Asset> Default for AssetCommands<A> {
    fn default() -> Self {
        Self { on_load: default() }
    }
}

impl<A: Asset> AssetCommands<A> {
    /// Defer a command until an asset is fully loaded.
    ///
    /// <warning>If the asset is already fully loaded, the command will never be executed.</warning>
    pub fn on_load(&mut self, id: impl Into<AssetId<A>>, command: impl Command) {
        let queue = self.on_load.entry(id.into()).or_default();
        queue.push(command);
    }

    /// Handle deferred commands arising from an asset load completion.
    ///
    /// TODO: TEST TEST TEST
    pub fn handle_events(
        mut this: ResMut<Self>,
        mut evs: EventReader<AssetEvent<A>>,
        mut commands: Commands,
    ) {
        for ev in evs.read() {
            match ev {
                AssetEvent::LoadedWithDependencies { id } => {
                    debug!("running deferred asset commands for {id:?}",);
                    if let Some(queue) = this.on_load.remove(id) {
                        commands.append(queue);
                    }
                }
                AssetEvent::Removed { id } => {
                    warn!("asset commands for {id:?} being dropped because the asset was removed",);
                    this.on_load.remove(id);
                }
                _ => (),
            }
        }
    }
}

/// Schedule for running commands deferred with [`AssetCommands`].
#[derive(ScheduleLabel)]
#[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct DeferredAssetCommands;

/// Plugin that initializes [`AssetCommands`] for a given asset type.
///
/// The deferred commands are run during the [`DeferredAssetCommands`] schedule by default.
#[derive(Copy, Clone, Debug)]
pub struct AssetCommandPlugin<A>(PhantomData<A>);

impl<A> Default for AssetCommandPlugin<A> {
    fn default() -> Self {
        Self(default())
    }
}

impl<A: Asset> Plugin for AssetCommandPlugin<A> {
    fn build(&self, app: &mut App) {
        app.init_resource::<AssetCommands<A>>()
            .init_schedule(DeferredAssetCommands)
            .add_systems(
                DeferredAssetCommands,
                AssetCommands::<A>::handle_events.run_if(on_event::<AssetEvent<A>>()),
            );

        app.world
            .resource_mut::<MainScheduleOrder>()
            .insert_after(AssetEvents, DeferredAssetCommands);
    }
}
