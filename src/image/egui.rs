use bevy::{asset::AssetPath, prelude::*};
use bevy_egui::{egui, EguiUserTextures};

use super::*;

#[cfg(feature = "egui")]
#[derive(Debug, Default, Copy, Clone, Component)]
pub struct EguiTextureId(pub egui::TextureId);

impl DrawImage {
    pub fn load_images(
        mut q: Query<(Entity, &mut DrawImage, Has<Sprite>, Has<EguiTextureId>), Changed<DrawImage>>,
        asset_server: Res<AssetServer>,
    ) {
        for (id, mut this, has_sprite, has_texture_id) in &mut q {
            if this.asset_handle.as_ref().map_or(true, |h| {
                h.path() != Some(&AssetPath::from_path(&this.path))
            }) {
                debug!("Loading image asset {} for {:?}", this.path.display(), id);
                this.asset_handle = Some(asset_server.load(AssetPath::from_path(&this.path)));
                if !has_sprite && !has_texture_id {
                    warn!("{:?} has a DrawImage but no Sprite or EguiTextureId", id);
                }
            }
        }
    }

    pub fn update_sprites(mut q: Query<(&DrawImage, &mut Sprite), Changed<DrawImage>>) {
        for (this, mut sprite) in &mut q {
            sprite.image = this
                .asset_handle
                .clone()
                .expect("load_sprites should have set our asset_handle already");
            sprite.custom_size = Some(this.size)
        }
    }

    pub fn update_texture_ids(
        mut q: Query<(&DrawImage, &mut EguiTextureId), Changed<DrawImage>>,
        asset_server: Res<AssetServer>,
        mut egui_textures: ResMut<EguiUserTextures>,
    ) {
        for (this, mut texture_id) in &mut q {
            let handle = this
                .asset_handle
                .clone()
                .expect("load_sprites should have set our asset_handle already");
            *texture_id = EguiTextureId(
                egui_textures
                    .image_id(&handle)
                    .unwrap_or_else(|| egui_textures.add_image(handle)),
            );
        }
    }
}
