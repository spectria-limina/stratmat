use std::sync::Arc;

use bevy::{ecs::system::SystemState, prelude::*};
use bevy_egui::egui;
use itertools::Itertools;

use crate::{
    egui::widget::{egui_context, Widget},
    waymark::window::WaymarkWindow,
};

#[derive(Component, Copy, Clone, Debug, Default)]
pub struct TopMenu;

pub fn show_top(world: &mut World) {
    let ctx = egui_context(world);
    let mut state = SystemState::<(
        Query<Entity, With<TopMenu>>,
        Query<&Widget>,
        Query<&Children>,
    )>::new(world);

    egui::TopBottomPanel::top("top").show(&ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            let (mut menu_q, widget_q, children_q) = state.get_mut(world);
            let id = menu_q.single_mut();

            for widget in widget_q
                .iter_many(children_q.children(id))
                .copied()
                .collect_vec()
            {
                widget.show_world(world, ui);
            }

            state.apply(world);
        })
    });
}

pub struct MenuPlugin;
impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.world_mut().spawn(TopMenu);
        app.add_systems(Update, show_top);
    }
}
pub fn plugin() -> MenuPlugin { MenuPlugin }
