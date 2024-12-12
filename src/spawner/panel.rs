use std::marker::PhantomData;

use bevy::prelude::*;
use bevy_egui::egui;

use super::{Spawnable, Spawner};
use crate::{
    ecs::{EntityWorldExts as _, NestedSystemExts},
    widget::{widget, InitWidget, WidgetCtx, WidgetSystemId},
};

#[derive(Component, derive_more::Debug, Reflect)]
#[require(InitWidget(|| widget!()))]
pub struct SpawnerPanel<T: Spawnable> {
    #[debug(skip)]
    _ph: PhantomData<T>,
}

impl<T: Spawnable> SpawnerPanel<T> {
    pub fn new() -> Self { Self { _ph: PhantomData } }

    pub fn show(WidgetCtx { ns: _ns, id, ui }: WidgetCtx, world: &mut World) {
        ui.add_space(T::sep().y);
        let frame = egui::Frame {
            outer_margin: egui::Margin::symmetric(T::sep().x, T::sep().y) / 2.0,
            ..default()
        };
        frame.show(ui, |ui| {
            ui.with_layout(
                egui::Layout::left_to_right(egui::Align::Min)
                    .with_main_wrap(true)
                    .with_main_align(egui::Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing = egui::Vec2::new(T::sep().x, T::sep().y);
                    let spawners: Vec<WidgetSystemId> = todo!();
                    for id in spawners {
                        world.run_nested_with(id, ui);
                    }
                },
            )
        });
    }
}
