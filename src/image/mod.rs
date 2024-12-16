use std::path::PathBuf;

use bevy::prelude::*;

#[cfg(feature = "dom")]
#[derive(bevy::asset::Asset, bevy::reflect::TypePath)]
pub struct Image;

#[cfg(feature = "egui")]
mod egui;
#[cfg(feature = "egui")]
pub use egui::*;
#[cfg(feature = "dom")]
mod dom;
#[cfg(feature = "dom")]
pub use dom::*;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Reflect)]
pub enum DrawImageKind {
    Sprite,
    Ui,
}

#[derive(Clone, Debug, Component, Reflect)]
#[component(on_insert = Self::on_insert)]
pub struct DrawImage {
    pub path: PathBuf,
    pub size: Vec2,
    pub kind: DrawImageKind,
    #[cfg(feature = "egui")]
    pub asset_handle: Option<Handle<Image>>,
}

impl DrawImage {
    pub fn new(path: PathBuf, size: Vec2, kind: DrawImageKind) -> Self {
        Self {
            path,
            size,
            kind,
            #[cfg(feature = "egui")]
            asset_handle: None,
        }
    }
}

pub struct ImagePlugin;

impl Plugin for ImagePlugin {
    fn build(&self, app: &mut App) {}
}

pub fn plugin() -> ImagePlugin { ImagePlugin }
