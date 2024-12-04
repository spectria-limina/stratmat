use bevy::prelude::*;

pub trait AssetHookExt {
    /// Runs a system once when the asset indicated by the provided
    /// handled is fully loaded.
    ///
    /// If the asset is already loaded, run it immediately.
    ///
    /// This may never be called, if the asset is unloaded before it
    /// finishes loading. A pending hook will not prevent an asset
    /// from being dropped if its refcount drops to 0. If you want to
    /// prevent that, use `on_asset_loaded_with` and pass the handle
    /// as an argument to the system.
    ///
    /// `on_asset_loaded` is commonly used with closures, but it does
    /// not work with closures that capture variables. Instead of using
    /// captures, use `on_asset_loaded_with`.
    fn on_asset_loaded<M, S, A, H>(&mut self, handle: H, system: S)
    where
        M: 'static,
        S: IntoSystem<(), (), M> + Send + Sync + 'static,
        A: Asset,
        H: Into<AssetId<A>>,
    {
        self.on_asset_loaded_with(handle, system, ())
    }

    /// Runs a system once when the asset indicated by the provided
    /// handled is fully loaded.
    ///
    /// This is identical to `on_asset_loaded` but it can also be
    /// passed system input.
    fn on_asset_loaded_with<I, M, S, A, H>(
        &mut self,
        handle: H,
        system: S,
        input: <I as SystemInput>::Inner<'static>,
    ) where
        // FIXME: These Sync bounds shouldn't be needed, but they end up
        // being somewhere in the guts of the implementation.
        I: SystemInput + Send + Sync + 'static,
        <I as SystemInput>::Inner<'static>: Send + Sync,
        M: 'static,
        S: IntoSystem<I, (), M> + Send + Sync + 'static,
        A: Asset,
        H: Into<AssetId<A>>;
}

impl AssetHookExt for World {
    fn on_asset_loaded_with<I, M, S, A, H>(
        &mut self,
        handle: H,
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
            (handle.into(), system, input),
        ) {
            error!("run deferred system error: {e}");
        }
    }
}

impl AssetHookExt for Commands<'_, '_> {
    fn on_asset_loaded_with<I, M, S, A, H>(
        &mut self,
        handle: H,
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
        self.run_system_cached_with(
            asset_loaded_run_impl::<I, M, S, A>,
            (handle.into(), system, input),
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
        if let Err(e) = world.run_system_cached_with(system, input) {
            error!("error running system after asset {asset_id} loaded: {e}");
        }
    } else {
        let id = world
            .spawn((
                OnLoadedHook {
                    target_id: asset_id,
                    command: Some(Box::new(move |commands: &mut Commands| {
                        commands.queue(move |world: &mut World| {
                            if let Err(e) = world.run_system_cached_with(system, input) {
                                error!("error running system after asset {asset_id} loaded: {e}");
                            }
                        })
                    })),
                },
                LifecycleUninitialized,
            ))
            .id();
        debug!("deferred OnLoad hook {id} for {asset_id}");
    }
}

/// This is basically a dynamic [`Command`], but because of the difficulty
/// moving unsized types out of boxes, we use [`FnOnce`]. The parameter is
/// a `&mut Commands` because it's a closure that queues itself.
type DynCommand = Box<dyn FnOnce(&mut Commands) + Send + Sync>;

#[derive(Component, TypePath)]
pub struct OnLoadedHook<A: Asset> {
    target_id: AssetId<A>,
    command: Option<DynCommand>,
}

pub fn handle_on_loaded<A: Asset>(
    mut q: Query<(Entity, &mut OnLoadedHook<A>)>,
    mut reader: EventReader<AssetEvent<A>>,
    mut commands: Commands,
) {
    for ev in reader.read() {
        match ev {
            AssetEvent::Added { id } => {
                debug!("asset added: {id}");
                for (hook_id, mut hook) in &mut q {
                    if *id == hook.target_id {
                        debug!("firing OnLoad hook {hook_id} targeting {}", hook.target_id);
                        if hook.command.is_some() {
                            hook.command.take().expect("we only take once")(&mut commands);
                        }
                        commands.entity(hook_id).despawn();
                    }
                }
            }
            AssetEvent::Removed { id } => {
                for (hook_id, hook) in &q {
                    if *id == hook.target_id {
                        commands.entity(hook_id).despawn();
                    }
                }
            }
            _ => {}
        }
    }

    // Clear uninitalized warnings on any hooks of our asset type.
    for (id, _) in &q {
        commands.entity(id).remove::<LifecycleUninitialized>();
    }
}

/// Complain about any remaining `LifecycleUninitialized` markers
/// after event handlers have removed any instances of them.
pub fn diagnose_uninitialized(
    q: Query<Entity, With<LifecycleUninitialized>>,
    mut commands: Commands,
) {
    for id in &q {
        error!("Lifecycle hook {id} is for an uninitialized asset type. It will never be called.");
        commands.entity(id).remove::<LifecycleUninitialized>();
    }
}

/// Marker struct used to complain about uninitialized asset types.
#[derive(Default, Copy, Clone, Debug, Reflect, Component)]
pub struct LifecycleUninitialized;

/// `SystemSet`s into which all the hooks are inserted.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[derive(SystemSet)]
pub enum Hooks {
    OnLoaded,
}

/// Extensions to `App` to allow registration of `Asset`s for lifecycle support.
///
/// FIXME: Things should not silently fail if not registered.
pub trait InitExt {
    /// Initialize an asset, including lifecycle features.
    fn init_asset_with_lifecycle<A: Asset>(&mut self) -> &mut Self;

    /// Initialize only the lifecycle. Use this for already-initialized external types.
    fn init_asset_lifecycle<A: Asset>(&mut self) -> &mut Self;
}

impl InitExt for App {
    fn init_asset_with_lifecycle<A: Asset>(&mut self) -> &mut Self {
        self.init_asset::<A>().init_asset_lifecycle::<A>()
    }

    fn init_asset_lifecycle<A: Asset>(&mut self) -> &mut Self {
        self.add_systems(PreUpdate, handle_on_loaded::<A>.in_set(Hooks::OnLoaded))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LifecyclePlugin;

impl Plugin for LifecyclePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, diagnose_uninitialized.after(Hooks::OnLoaded));
    }
}

pub fn plugin() -> LifecyclePlugin {
    LifecyclePlugin
}
