use std::path::PathBuf;

use bevy::prelude::*;

mod lifecycle;
mod listing;

pub use lifecycle::*;
pub use listing::*;

/// This probably gets initialized wrong on non-wasm32 platforms.
#[derive(Clone, Deref, Debug, Reflect, Default, Resource)]
pub struct RootAssetPath(pub PathBuf);
