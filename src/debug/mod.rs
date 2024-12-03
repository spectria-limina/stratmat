use bevy::prelude::*;

pub fn log_asset_events<A: Asset>(
    mut reader: EventReader<AssetEvent<A>>,
    asset_server: Res<AssetServer>,
) {
    for ev in reader.read() {
        // This is info because it's already debug controlled.
        let id = match ev {
            AssetEvent::Added { id } => id,
            AssetEvent::Modified { id } => id,
            AssetEvent::Removed { id } => id,
            AssetEvent::Unused { id } => id,
            AssetEvent::LoadedWithDependencies { id } => id,
        };
        info!(
            "asset event {ev:?} on path '{:?}'",
            asset_server.get_path(*id)
        );
    }
}

pub fn log_events<E: Event + std::fmt::Debug>(mut reader: EventReader<E>) {
    for ev in reader.read() {
        info!("asset event {ev:?}");
    }
}
