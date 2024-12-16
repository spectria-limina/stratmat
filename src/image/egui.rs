use bevy::{
    asset::AssetPath,
    ecs::{component::ComponentId, world::DeferredWorld},
    prelude::*,
};
use bevy_egui::{egui, EguiUserTextures};
use itertools::Itertools;

use super::*;

#[derive(Debug, Copy, Clone, Component)]
pub struct EguiTextureId(pub egui::TextureId);

impl DrawImage {
    pub fn on_insert(mut world: DeferredWorld, id: Entity, _: ComponentId) {
        let this = world
            .get::<DrawImage>(id)
            .expect("I must exist in my own hook");
        let kind = this.kind;
        let mut commands = world.commands();

        commands.run_system_cached_with(Self::load_images_for, id);
    }

    pub fn load_images_for(
        In(id): In<Entity>,
        mut q: Query<
            (
                Entity,
                &mut DrawImage,
                Option<&Sprite>,
                Option<&EguiTextureId>,
            ),
            Changed<DrawImage>,
        >,
        asset_server: Res<AssetServer>,
        mut egui_textures: ResMut<EguiUserTextures>,
        mut commands: Commands,
    ) {
        let Ok((id, mut this, sprite, texture_id)) = q.get_mut(id) else {
            warn!("load_images_for on {:?} could not get the DrawImage", id);
            return;
        };

        debug!("Loading image asset {} for {:?}", this.path.display(), id);
        let handle = asset_server.load(AssetPath::from_path(&this.path));
        this.asset_handle = Some(handle.clone());

        match this.kind {
            DrawImageKind::Sprite => {
                let mut sprite = sprite.cloned().unwrap_or_default();
                sprite.image = handle;
                sprite.custom_size = Some(this.size);
                commands.entity(id).insert(sprite);
            }
            DrawImageKind::Ui => {
                let texture_id = EguiTextureId(
                    egui_textures
                        .image_id(&handle)
                        .unwrap_or_else(|| egui_textures.add_image(handle)),
                );
                commands.entity(id).insert(texture_id);
            }
        }
    }
}
