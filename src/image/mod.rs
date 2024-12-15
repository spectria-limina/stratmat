#[cfg(feature = "dom")]
#[derive(bevy::asset::Asset, bevy::reflect::TypePath)]
pub struct Image;

#[cfg(feature = "egui")]
pub use bevy::image::Image;
