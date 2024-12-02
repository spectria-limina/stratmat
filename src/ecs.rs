use bevy::ecs::prelude::*;
use bevy::prelude::*;

pub trait AssetCommandsExt {
    fn run_system_when_asset_loaded_with<I, M, S, A, H>(
        &mut self,
        id: H,
        system: S,
        input: <I as SystemInput>::Inner<'static>,
    ) where
        // FIXME: These Sync bounds shouldn't be needed, but they end up being somewhere in the guts of the implementation.
        I: SystemInput + Send + Sync + 'static,
        <I as SystemInput>::Inner<'static>: Send + Sync,
        M: 'static,
        S: IntoSystem<I, (), M> + Send + Sync + 'static,
        A: Asset,
        H: Into<AssetId<A>>;

    fn run_system_when_asset_loaded<M, S, A, H>(&mut self, id: H, system: S)
    where
        M: 'static,
        S: IntoSystem<(), (), M> + Send + Sync + 'static,
        A: Asset,
        H: Into<AssetId<A>>,
    {
        self.run_system_when_asset_loaded_with(id, system, ())
    }
}

impl AssetCommandsExt for World {
    fn run_system_when_asset_loaded_with<I, M, S, A, H>(
        &mut self,
        id: H,
        system: S,
        input: <I as SystemInput>::Inner<'static>,
    ) where
        I: SystemInput + Send + Sync + 'static,
        <I as SystemInput>::Inner<'static>: Send + Sync,
        M: 'static,
        S: IntoSystem<I, (), M> + Send + Sync + 'static,
        A: Asset,
        H: Into<AssetId<A>>,
    {
        if let Err(e) = self.run_system_cached_with(
            asset_loaded_run_impl::<I, M, S, A>,
            (id.into(), system, input),
        ) {
            error!("run deferred system error: {e}");
        }
    }
}

impl AssetCommandsExt for Commands<'_, '_> {
    fn run_system_when_asset_loaded_with<I, M, S, A, H>(
        &mut self,
        id: H,
        system: S,
        input: <I as SystemInput>::Inner<'static>,
    ) where
        // TODO: These Sync bounds shouldn't be needed, but they end up being somewhere in the guts of the implementation.
        I: SystemInput + Send + Sync + 'static,
        <I as SystemInput>::Inner<'static>: Send + Sync,
        M: 'static,
        S: IntoSystem<I, (), M> + Send + Sync + 'static,
        A: Asset,
        H: Into<AssetId<A>>,
    {
        self.run_system_cached_with(
            asset_loaded_run_impl::<I, M, S, A>,
            (id.into(), system, input),
        )
    }
}

fn asset_loaded_run_impl<I, M, S, A>(
    In((asset_id, system, input)): In<(AssetId<A>, S, <I as SystemInput>::Inner<'static>)>,
    world: &mut World,
) where
    I: SystemInput + Send + Sync + 'static,
    <I as SystemInput>::Inner<'static>: Send + Sync,
    M: 'static,
    S: IntoSystem<I, (), M> + Send + Sync + 'static,
    A: Asset,
{
    let assets = world.get_resource::<Assets<A>>();
    if assets.and_then(|assets| assets.get(asset_id)).is_some() {
        debug!("immediately running system because {asset_id} is loaded",);
        if let Err(e) = world.run_system_cached_with(system, input) {
            error!("error running system after asset {asset_id} loaded: {e}");
        }
    } else {
        debug!("deferring system execution until asset {asset_id} is loaded");
        // Make sure that the closure is FnMut by managing the lifetime of the system and input.
        let mut sysinput = Some((system, input));
        world.spawn(
            (Name::new(format!("Asset load deferral observer [{asset_id}]")),
            // FIXME: This must take Commands and not World or it will cause a panic: bevyengine/bevy#14507
            Observer::new(move |trigger: Trigger<AssetEvent<A>>, mut commands: Commands| {
                debug!("deferred system checking asset events...");
                if sysinput.is_none() {
                    // This is harmless: we did our job and are just burning through queued events now.
                    return;
                }
                match trigger.event() {
                    AssetEvent::LoadedWithDependencies { id: target_id } => {
                        if &asset_id == target_id {
                            debug!("running deferred system for asset load on asset {asset_id}",);
                            let (system, input) = sysinput.take().unwrap();
                            commands.run_system_cached_with(system, input);
                            commands.entity(trigger.observer()).despawn();
                        }
                    }
                    AssetEvent::Removed { id: target_id } => {
                        if &asset_id == target_id {
                        debug!("deferred system for asset {asset_id} dropped becasue the asset was removed",);
                            commands.entity(trigger.observer()).despawn();
                        }
                    }
                    _ => (),
                }
        })));
    }
}

pub fn trigger_all_events<E: Event + Clone>(world: &mut World) {
    match world
        .run_system_cached(|mut reader: EventReader<E>| reader.read().cloned().collect::<Vec<_>>())
    {
        Ok(evs) => {
            for ev in evs {
                world.trigger(ev);
            }
        }
        Err(e) => {
            error!(
                "unable to trigger events of type {}: {e}",
                std::any::type_name::<E>()
            );
        }
    }
}
