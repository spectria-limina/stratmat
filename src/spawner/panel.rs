use bevy::prelude::*;
use bevy_egui::egui;
use type_variance::Invariant;

use super::{Spawnable, Spawner};
use crate::{exts::*, widget::WidgetWith};

#[derive(Component)]
pub struct SpawnerPanel<Target> {
    pub spacing: Vec2,
    pub spawners: Vec<Entity>,
    _t: Invariant<Target>,
}

impl<Target: Spawnable> SpawnerPanel<Target> {
    pub fn new(spacing: Vec2, spawners: impl IntoIterator<Item = Entity>) -> Self {
        Self {
            spacing,
            spawners: spawners.into_iter().collect(),
            _t: default(),
        }
    }

    pub fn show(WidgetWith(_id, ui, this): WidgetWith<Self>, world: &mut World) {
        ui.add_space(this.spacing.y);
        let frame = egui::Frame {
            outer_margin: egui::Margin::symmetric(this.spacing.x, this.spacing.y) / 2.0,
            ..default()
        };
        frame.show(ui, |ui| {
            ui.with_layout(
                egui::Layout::left_to_right(egui::Align::Min)
                    .with_main_wrap(true)
                    .with_main_align(egui::Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing = egui::Vec2::new(this.spacing.x, this.spacing.y);
                    for &id in &this.spawners {
                        world
                            .entity_mut(id)
                            .run_instanced_with(Spawner::<Target>::show, ui);
                    }
                },
            )
        });
    }
}
