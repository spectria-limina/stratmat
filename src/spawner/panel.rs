use std::{any::type_name, marker::PhantomData};

use bevy::prelude::*;
use bevy_egui::egui;

use super::{Spawnable, Spawner};
use crate::{
    ecs::{EntityWorldExts as _, NestedSystemExts},
    widget::{widget, InitWidget, Widget, WidgetCtx, WidgetSystemId},
};

#[derive(Component, derive_more::Debug, Reflect)]
#[require(InitWidget(|| widget!()))]
pub struct SpawnerPanel<T: Spawnable> {
    #[debug(skip)]
    _ph: PhantomData<T>,
}

impl<T: Spawnable> SpawnerPanel<T> {
    pub fn new() -> Self { Self { _ph: PhantomData } }

    pub fn show(
        WidgetCtx { ns, id, ui }: WidgetCtx,
        spawner_q: Query<&Widget, With<Spawner<T>>>,
        children_q: Query<&Children>,
    ) {
        debug!("Drawing SpawnerPanel<{:?}>", type_name::<T>());
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
                    for spawner in spawner_q.iter_many(children_q.children(id)) {
                        spawner.show(ns, ui);
                    }
                },
            )
        });
    }
}
