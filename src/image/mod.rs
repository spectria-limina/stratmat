use std::path::PathBuf;

use bevy::prelude::*;

#[cfg(feature = "dom")]
#[derive(bevy::asset::Asset, bevy::reflect::TypePath)]
pub struct Image;

#[cfg(feature = "egui")]
mod egui;
#[cfg(feature = "egui")]
pub use egui::*;

#[derive(Clone, Debug, Component, Reflect)]
pub struct DrawImage {
    pub path: PathBuf,
    pub size: Vec2,
    #[cfg(feature = "egui")]
    pub asset_handle: Option<Handle<Image>>,
}

impl DrawImage {
    pub fn new(path: PathBuf, size: Vec2) -> Self {
        Self {
            path,
            size,
            #[cfg(feature = "egui")]
            asset_handle: None,
        }
    }
}

pub struct ImagePlugin;

impl Plugin for ImagePlugin {
    fn build(&self, app: &mut App) {
        #[cfg(feature = "egui")]
        app.add_systems(PreUpdate, DrawImage::load_images)
            .add_systems(
                PreUpdate,
                (DrawImage::update_sprites, DrawImage::update_texture_ids)
                    .after(DrawImage::load_images),
            );
    }
}

pub fn plugin() -> ImagePlugin { ImagePlugin }
