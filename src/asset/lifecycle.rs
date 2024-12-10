use std::{any::type_name, marker::PhantomData, ops::Deref, panic::Location};

use bevy::{
    asset::AssetPath,
    ecs::system::{ReadOnlySystemParam, SystemParam, SystemState},
    prelude::*,
    ptr::Ptr,
};
use derive_more::derive::Into;

#[derive(Deref, Resource, Debug)]
struct AssetHookTargetHandle<A: Asset>(Handle<A>);

#[derive(SystemParam)]
pub struct AssetHookTargetState<'w, A: Asset> {
    assets: Res<'w, Assets<A>>,
    handle: Res<'w, AssetHookTargetHandle<A>>,
}

#[derive(Debug)]
pub struct AssetHookTarget<'a, A: Asset> {
    pub asset: &'a A,
    pub handle: Handle<A>,
}

impl<A: Asset> Deref for AssetHookTarget<'_, A> {
    type Target = A;

    fn deref(&self) -> &Self::Target { self.asset }
}

unsafe impl<'a, A: Asset> SystemParam for AssetHookTarget<'a, A> {
    type State = <AssetHookTargetState<'a, A> as SystemParam>::State;
    type Item<'world, 'state> = AssetHookTarget<'world, A>;

    fn init_state(
        world: &mut World,
        system_meta: &mut bevy::ecs::system::SystemMeta,
    ) -> Self::State {
        AssetHookTargetState::init_state(world, system_meta)
    }

    unsafe fn get_param<'world, 'state>(
        state: &'state mut Self::State,
        system_meta: &bevy::ecs::system::SystemMeta,
        world: bevy::ecs::world::unsafe_world_cell::UnsafeWorldCell<'world>,
        change_tick: bevy::ecs::component::Tick,
    ) -> Self::Item<'world, 'state> {
        let AssetHookTargetState { assets, handle } =
            AssetHookTargetState::get_param(state, system_meta, world, change_tick);
        AssetHookTarget {
            asset: assets
                .into_inner()
                .get(handle.id())
                .expect("Asset should be alive during asset hook for it"),
            handle: handle.clone(),
        }
    }
}

unsafe impl<A: Asset> ReadOnlySystemParam for AssetHookTarget<'_, A> {}

pub trait AssetHookExt {
    /// Runs a system once when the asset indicated by the provided
    /// handled is fully loaded.
    ///
    /// If the asset is already loaded, run it immediately.
    ///
    /// This may never be called, if the asset is unloaded before it
    /// finishes loading. A pending hook will hold the handle, which will
    /// prevent the asset from being dropped if and only if it is strong.
    ///
    /// `on_asset_loaded` is commonly used with closures, but it does
    /// not work with closures that capture variables. Instead of using
    /// captures, use `on_asset_loaded_with`.
    ///
    /// The system can refer to the [`AssetHookTarget`] as a parameter
    /// to access the loaded resource.
    #[track_caller]
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
    #[track_caller]
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
    #[track_caller]
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
    #[track_caller]
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

#[track_caller]
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
    if !world.contains_resource::<LifecycleRegistration<A>>() {
        panic!(
            "{} must be registered with init_lifecycle before calling on_asset_loaded",
            type_name::<A>()
        )
    }
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
            .spawn(OnLoadedHook {
                target: handle.clone(),
                caller: Location::caller(),
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
            })
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
    caller: &'static Location<'static>,
    command: Option<DynCommand>,
}

pub fn handle_on_loaded<A: Asset>(world: &mut World) {
    // Run as an exclusive system because we are going to be putting
    // the target in as a resource and don't want it messed with by
    // interleaving commands.
    let mut state = SystemState::<(
        Query<(Entity, &'static mut OnLoadedHook<A>)>,
        EventReader<AssetEvent<A>>,
        ResMut<Assets<A>>,
        Commands,
    )>::new(world);
    let (mut q, mut reader, mut assets, mut commands) = state.get_mut(world);

    for ev in reader.read() {
        match *ev {
            AssetEvent::Added { id } => {
                debug!("asset added: {id}");
                let Some(handle) = assets.get_strong_handle(id) else {
                    // We will warn about this situation when we get to the Removed event handler.
                    continue;
                };
                commands.insert_resource(AssetHookTargetHandle(handle));
                for (hook_id, mut hook) in &mut q {
                    if id == hook.target.id() {
                        debug!(
                            "{}: firing OnLoad hook {hook_id} targeting {}",
                            hook.caller,
                            hook.target.id()
                        );
                        if hook.command.is_some() {
                            hook.command.take().expect("we only take once")(&mut commands);
                        }
                        commands.entity(hook_id).despawn();
                    }
                }
                commands.remove_resource::<AssetHookTargetHandle<A>>();
            }
            AssetEvent::Removed { id } => {
                for (hook_id, hook) in &q {
                    if id == hook.target.id() {
                        warn!(
                            "{}: asset {} removed before on_loaded hook could fire",
                            hook.caller, id
                        );
                        commands.entity(hook_id).despawn();
                    }
                }
            }
            _ => {}
        }
    }

    state.apply(world);
}

/// `SystemSet`s into which all the hooks are inserted.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[derive(SystemSet)]
pub enum Systems {
    GlobalAssets,
    Hooks,
    OnLoaded,
}

#[derive(SystemParam)]
pub struct GlobalAssetState<'w, A: Asset> {
    // INVARIANT: The order of fields must match the order in validate_param below.
    assets: Res<'w, Assets<A>>,
    handle: Res<'w, GlobalAssetHandle<A>>,
}

/// A [`SystemParam`] containing a copy of a single global asset.
#[derive(Deref, Debug)]
pub struct GlobalAsset<'a, A: Asset>(&'a A);

unsafe impl<'a, A: Asset> SystemParam for GlobalAsset<'a, A> {
    type State = <GlobalAssetState<'a, A> as SystemParam>::State;
    type Item<'world, 'state> = GlobalAsset<'world, A>;

    fn init_state(
        world: &mut World,
        system_meta: &mut bevy::ecs::system::SystemMeta,
    ) -> Self::State {
        GlobalAssetState::init_state(world, system_meta)
    }

    unsafe fn get_param<'world, 'state>(
        state: &'state mut Self::State,
        system_meta: &bevy::ecs::system::SystemMeta,
        world: bevy::ecs::world::unsafe_world_cell::UnsafeWorldCell<'world>,
        change_tick: bevy::ecs::component::Tick,
    ) -> Self::Item<'world, 'state> {
        let GlobalAssetState { assets, handle } =
            GlobalAssetState::get_param(state, system_meta, world, change_tick);
        GlobalAsset(assets.into_inner().get(handle.id()).unwrap_or_else(|| {
            panic!(
                "GlobalAsset<{}> param fetched but asset '{}' is not loaded",
                type_name::<A>(),
                (**handle).path().unwrap_or(&default()),
            )
        }))
    }

    unsafe fn validate_param(
        state: &Self::State,
        _system_meta: &bevy::ecs::system::SystemMeta,
        world: bevy::ecs::world::unsafe_world_cell::UnsafeWorldCell,
    ) -> bool {
        // INVARIANT: This *must* match the order of fields in GlobalAssetState.
        let (asset_id, handle_id) = state.state;

        // SAFETY: We have a Res<Assets<A>> in our state, so it will set up the necessary accesses.
        //         Furthermore, since we got the ComponentId via Res<Assets<A>>, Ptr::deref is valid.
        // IMPORTANT: We *must* use get_resource_by_id here, in case somehow the resource was moved to a different ID.
        let assets: Option<&Assets<A>> =
            unsafe { world.get_resource_by_id(asset_id).map(|p| Ptr::deref(p)) };
        // SAFETY: Likewise for Res<GlobalAssetHandle<A>>
        let handle: Option<&GlobalAssetHandle<A>> =
            unsafe { world.get_resource_by_id(handle_id).map(|p| Ptr::deref(p)) };

        // We want to fail if the Assets<A> is not present, so return true in that case.
        assets.is_none_or(|assets| handle.is_some_and(|handle| assets.contains(handle)))
    }
}

unsafe impl<A: Asset> ReadOnlySystemParam for GlobalAsset<'_, A> {}

/// This is a [`SystemParam`] that's just an `Option<GlobalAsset>`, but working around coherence issues.
#[derive(Deref, Into, Debug)]
pub struct OptionalGlobalAsset<'a, A: Asset>(Option<GlobalAsset<'a, A>>);

impl<'a, A: Asset> OptionalGlobalAsset<'a, A> {
    pub fn option(&self) -> &Option<GlobalAsset<'a, A>> { self.deref() }

    pub fn into_option(self) -> Option<GlobalAsset<'a, A>> { self.into() }
}

unsafe impl<'a, A: Asset> SystemParam for OptionalGlobalAsset<'a, A> {
    type State = <GlobalAsset<'a, A> as SystemParam>::State;
    type Item<'world, 'state> = OptionalGlobalAsset<'world, A>;

    fn init_state(
        world: &mut World,
        system_meta: &mut bevy::ecs::system::SystemMeta,
    ) -> Self::State {
        GlobalAsset::init_state(world, system_meta)
    }

    unsafe fn get_param<'world, 'state>(
        state: &'state mut Self::State,
        system_meta: &bevy::ecs::system::SystemMeta,
        world: bevy::ecs::world::unsafe_world_cell::UnsafeWorldCell<'world>,
        change_tick: bevy::ecs::component::Tick,
    ) -> Self::Item<'world, 'state> {
        OptionalGlobalAsset(
            GlobalAsset::validate_param(state, system_meta, world)
                .then(|| GlobalAsset::get_param(state, system_meta, world, change_tick)),
        )
    }
}
unsafe impl<A: Asset> ReadOnlySystemParam for OptionalGlobalAsset<'_, A> {}

#[derive(Component, Clone, derive_more::Debug, Reflect, Deref)]
pub struct GlobalAssetPath<A: Asset>(#[deref] AssetPath<'static>, #[debug(skip)] PhantomData<A>);

impl<A: Asset> GlobalAssetPath<A> {
    pub fn new<'a>(path: impl Into<AssetPath<'a>>) -> Self {
        Self(path.into().into_owned(), PhantomData)
    }
}

#[derive(Resource, Debug, Reflect, Deref)]
pub struct GlobalAssetHandle<A: Asset>(Handle<A>);

impl<'a, A: Asset> From<&'a GlobalAssetHandle<A>> for AssetId<A> {
    fn from(value: &'a GlobalAssetHandle<A>) -> Self { Self::from(&value.0) }
}

// FIXME: This clones the asset data.
pub fn load_global_assets<A: Asset>(
    q: Query<(Entity, &GlobalAssetPath<A>)>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    for (id, path) in &q {
        let GlobalAssetPath(ref path, _ph) = *path;
        debug!("Loading global from {}", path);
        let target = asset_server.load::<A>(path.clone());

        commands.entity(id).despawn();
        commands.on_asset_loaded(
            target.clone(),
            move |target: AssetHookTarget<A>, mut commands: Commands| {
                commands.insert_resource(GlobalAssetHandle(target.handle));
            },
        );
    }
}

/// Extensions to `App` to allow registration of `Asset`s for lifecycle support.
pub trait LifecycleExts {
    /// Initialize an asset, including lifecycle features.
    ///
    /// FIXME: Remove Clone bound.
    fn init_asset_with_lifecycle<A: Asset>(&mut self) -> &mut Self;

    /// Initialize only the lifecycle. Use this for already-initialized external types.
    fn init_lifecycle<A: Asset>(&mut self) -> &mut Self;

    fn load_global_asset<'a, A: Asset>(&mut self, path: impl Into<AssetPath<'a>>) -> &mut Self;
}

impl LifecycleExts for App {
    fn init_asset_with_lifecycle<A: Asset>(&mut self) -> &mut Self {
        self.init_asset::<A>().init_lifecycle::<A>()
    }

    fn init_lifecycle<A: Asset>(&mut self) -> &mut Self {
        self.init_resource::<LifecycleRegistration<A>>()
            .add_systems(PreUpdate, handle_on_loaded::<A>.in_set(Systems::OnLoaded))
            .add_systems(
                PreUpdate,
                load_global_assets::<A>.in_set(Systems::GlobalAssets),
            )
    }

    fn load_global_asset<'a, A: Asset>(&mut self, path: impl Into<AssetPath<'a>>) -> &mut Self {
        let world = self.world_mut();
        if !world.contains_resource::<LifecycleRegistration<A>>() {
            panic!(
                "{} must be registered with init_lifecycle() before using load_global_asset()",
                type_name::<A>()
            );
        }
        world.spawn(GlobalAssetPath::<A>::new(path));
        self
    }
}

/// Marker resource to indicate that an asset type has had lifecycle functionality registered.
#[derive(Resource, Debug, Copy, Clone, Reflect)]
pub struct LifecycleRegistration<A> {
    _ph: PhantomData<A>,
}

impl<A> Default for LifecycleRegistration<A> {
    fn default() -> Self { Self { _ph: PhantomData } }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LifecyclePlugin;

impl Plugin for LifecyclePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(PreUpdate, Systems::OnLoaded.in_set(Systems::Hooks))
            .configure_sets(PreUpdate, (Systems::GlobalAssets, Systems::Hooks).chain());
    }
}

pub fn plugin() -> LifecyclePlugin { LifecyclePlugin }
