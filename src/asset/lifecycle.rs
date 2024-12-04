use bevy::{
    ecs::system::{SystemParam, SystemState},
    prelude::*,
};
use std::ops::Deref;

/// The ID of an asset stored as a resource for [`AssetHookTarget`].
#[derive(Resource, Debug)]
pub struct AssetHookTargetId<A: Asset>(AssetId<A>);

#[derive(SystemParam)]
pub struct AssetHookTarget<'w, A: Asset> {
    assets: Res<'w, Assets<A>>,
    target: Res<'w, AssetHookTargetId<A>>,
}

impl<A: Asset> AssetHookTarget<'_, A> {
    fn get(this: &Self) -> &A {
        this.assets
            .get(this.target.0)
            .expect("Asset must be loaded")
    }
}

impl<A: Asset> Deref for AssetHookTarget<'_, A> {
    type Target = A;

    fn deref(&self) -> &Self::Target {
        Self::get(self)
    }
}

pub trait AssetHookExt {
    /// Runs a system once when the asset indicated by the provided
    /// handled is fully loaded.
    ///
    /// If the asset is already loaded, run it immediately.
    ///
    /// This may never be called, if the asset is unloaded before it
    /// finishes loading. A pending hook will hold the handle, which will
    /// prevent the asset from being dropped if it is strong.
    ///
    /// `on_asset_loaded` is commonly used with closures, but it does
    /// not work with closures that capture variables. Instead of using
    /// captures, use `on_asset_loaded_with`.
    ///
    /// The system can refer to the [`AssetHookTarget`] as a parameter
    /// to access the loaded resource.
    fn on_asset_loaded<M, S, A>(&mut self, handle: Handle<A>, system: S)
    where
        M: 'static,
        S: IntoSystem<(), (), M> + Send + Sync + 'static,
        A: Asset,
    {
        self.on_asset_loaded_with(handle, system, ())
    }

    /// Runs a system once when the asset indicated by the provided
    /// handled is fully loaded.
    ///
    /// This is identical to `on_asset_loaded` but it can also be
    /// passed system input.
    fn on_asset_loaded_with<I, M, S, A>(
        &mut self,
        handle: Handle<A>,
        system: S,
        input: <I as SystemInput>::Inner<'static>,
    ) where
        // FIXME: These Sync bounds shouldn't be needed, but they end up
        // being somewhere in the guts of the implementation.
        I: SystemInput + Send + Sync + 'static,
        <I as SystemInput>::Inner<'static>: Send + Sync,
        M: 'static,
        S: IntoSystem<I, (), M> + Send + Sync + 'static,
        A: Asset;
}

impl AssetHookExt for World {
    fn on_asset_loaded_with<I, M, S, A>(
        &mut self,
        handle: Handle<A>,
        system: S,
        input: <I as SystemInput>::Inner<'static>,
    ) where
        I: SystemInput + Send + Sync + 'static,
        <I as SystemInput>::Inner<'static>: Send + Sync,
        M: 'static,
        S: IntoSystem<I, (), M> + Send + Sync + 'static,
        A: Asset,
    {
        if let Err(e) = self
            .run_system_cached_with(asset_loaded_run_impl::<I, M, S, A>, (handle, system, input))
        {
            error!("run deferred system error: {e}");
        }
    }
}

impl AssetHookExt for Commands<'_, '_> {
    fn on_asset_loaded_with<I, M, S, A>(
        &mut self,
        handle: Handle<A>,
        system: S,
        input: <I as SystemInput>::Inner<'static>,
    ) where
        I: SystemInput + Send + Sync + 'static,
        <I as SystemInput>::Inner<'static>: Send + Sync,
        M: 'static,
        S: IntoSystem<I, (), M> + Send + Sync + 'static,
        A: Asset,
    {
        self.run_system_cached_with(asset_loaded_run_impl::<I, M, S, A>, (handle, system, input))
    }
}

fn asset_loaded_run_impl<I, M, S, A>(
    In((handle, system, input)): In<(Handle<A>, S, <I as SystemInput>::Inner<'static>)>,
    world: &mut World,
) where
    I: SystemInput + Send + Sync + 'static,
    <I as SystemInput>::Inner<'static>: Send + Sync,
    M: 'static,
    S: IntoSystem<I, (), M> + Send + Sync + 'static,
    A: Asset,
{
    let assets = world.resource::<Assets<A>>();
    if assets.get(&handle).is_some() {
        if let Err(e) = world.run_system_cached_with(system, input) {
            error!(
                "error running system after asset {} loaded: {e}",
                handle.id()
            );
        }
    } else {
        let target_id = handle.id();
        let id = world
            .spawn((
                OnLoadedHook {
                    target: handle.clone(),
                    command: Some(Box::new(move |commands: &mut Commands| {
                        commands.queue(move |world: &mut World| {
                            if let Err(e) = world.run_system_cached_with(system, input) {
                                error!(
                                    "error running system after asset {} loaded: {e}",
                                    handle.id()
                                );
                            }
                        })
                    })),
                },
                LifecycleUninitialized,
            ))
            .id();
        debug!("deferred OnLoad hook {id} for {target_id}");
    }
}

/// This is basically a dynamic [`Command`], but because of the difficulty
/// moving unsized types out of boxes, we use [`FnOnce`]. The parameter is
/// a `&mut Commands` because it's a closure that queues itself.
type DynCommand = Box<dyn FnOnce(&mut Commands) + Send + Sync>;

#[derive(Component, TypePath)]
pub struct OnLoadedHook<A: Asset> {
    target: Handle<A>,
    command: Option<DynCommand>,
}

pub fn handle_on_loaded<A: Asset>(world: &mut World) {
    // Run as an exclusive system because we are going to be putting
    // the target in as a resource and don't want it messed with by
    // interleaving commands.
    let mut state = SystemState::<(
        Query<(Entity, &'static mut OnLoadedHook<A>)>,
        EventReader<AssetEvent<A>>,
        Commands,
    )>::new(world);
    let (mut q, mut reader, mut commands) = state.get_mut(world);

    for ev in reader.read() {
        match ev {
            AssetEvent::Added { id } => {
                debug!("asset added: {id}");
                commands.insert_resource(AssetHookTargetId(*id));
                for (hook_id, mut hook) in &mut q {
                    if *id == hook.target.id() {
                        debug!(
                            "firing OnLoad hook {hook_id} targeting {}",
                            hook.target.id()
                        );
                        if hook.command.is_some() {
                            hook.command.take().expect("we only take once")(&mut commands);
                        }
                        commands.entity(hook_id).despawn();
                    }
                }
                commands.remove_resource::<AssetHookTargetId<A>>();
            }
            AssetEvent::Removed { id } => {
                for (hook_id, hook) in &q {
                    if *id == hook.target.id() {
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

    state.apply(world);
}

/// Complain about any remaining `LifecycleUninitialized` markers
/// after event handlers have removed any instances of them.
pub fn diagnose_uninitialized(
    q: Query<Entity, With<LifecycleUninitialized>>,
    mut commands: Commands,
) {
    for id in &q {
        error!("Lifecycle features {id} is for an uninitialized asset type. It will never be called/loaded/etc.");
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

/// A [`Component`] containing a copy of a single global asset.
#[derive(Deref, Component, Copy, Clone, Debug, Reflect)]
pub struct GlobalAsset<A: Asset>(A);

#[derive(Component, Clone, Debug, TypePath)]
pub enum GlobalAssetLoader<A: Asset> {
    Unloaded(String),
    Loading(Handle<A>),
}

impl<A: Asset> GlobalAssetLoader<A> {
    fn new(path: String) -> Self {
        Self::Unloaded(path)
    }
}

// FIXME: This clones the asset data.
pub fn load_global_assets<A: Asset>(
    mut q: Query<(Entity, &mut GlobalAssetLoader<A>)>,
    mut assets: ResMut<Assets<A>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    // We don't use the on_loaded hook because we are removing the asset.
    for (id, mut loader) in &mut q {
        match *loader {
            GlobalAssetLoader::Unloaded(ref path) => {
                debug!("Loading global from {}", path);
                let handle = asset_server.load::<A>(path.clone());
                commands.entity(id).remove::<LifecycleUninitialized>();
                *loader = GlobalAssetLoader::Loading(handle)
            }
            GlobalAssetLoader::Loading(ref handle) => {
                if let Some(asset) = assets.remove(handle) {
                    debug!("Loading complete from {}", handle.path().unwrap());
                    commands.spawn(GlobalAsset(asset));
                    commands.entity(id).despawn();
                }
            }
        }
    }
}

/// Extensions to `App` to allow registration of `Asset`s for lifecycle support.
pub trait LifecycleExts {
    /// Initialize an asset, including lifecycle features.
    ///
    /// FIXME: Remove Clone bound.
    fn init_asset_with_lifecycle<A: Asset>(&mut self) -> &mut Self;

    /// Initialize only the lifecycle. Use this for already-initialized external types.
    fn init_asset_lifecycle<A: Asset>(&mut self) -> &mut Self;

    fn load_global_asset<A: Asset>(&mut self, path: &str) -> &mut Self;
}

impl LifecycleExts for App {
    fn init_asset_with_lifecycle<A: Asset>(&mut self) -> &mut Self {
        self.init_asset::<A>().init_asset_lifecycle::<A>()
    }

    fn init_asset_lifecycle<A: Asset>(&mut self) -> &mut Self {
        self.add_systems(PreUpdate, handle_on_loaded::<A>.in_set(Hooks::OnLoaded))
            .add_systems(
                PreUpdate,
                load_global_assets::<A>
                    .after(Hooks::OnLoaded)
                    .before(diagnose_uninitialized),
            )
    }

    fn load_global_asset<A: Asset>(&mut self, path: &str) -> &mut Self {
        self.world_mut().spawn((
            GlobalAssetLoader::<A>::new(path.into()),
            LifecycleUninitialized,
        ));
        self
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
