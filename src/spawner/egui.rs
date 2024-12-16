use bevy::prelude::*;
use bevy_egui::egui;

use super::*;
use crate::image::EguiTextureId;

impl<T: Spawnable> Spawner<T> {
    pub fn show(
        WidgetCtx { ns: _ns, id, ui }: WidgetCtx,
        spawner_q: Query<(&Spawner<T>, &EguiTextureId)>,
        mut pointer_ev: EventWriter<PointerHits>,
    ) {
        let (spawner, texture_id) = spawner_q
            .get(id)
            .expect("Spawner::show called without a Spawner");
        let resp = ui.add(
            egui::Image::new((texture_id.0, egui::Vec2::new(T::size().x, T::size().y)))
                .tint(egui::Color32::from_white_alpha(if spawner.enabled {
                    SPAWNER_ALPHA
                } else {
                    SPAWNER_DISABLED_ALPHA
                }))
                .sense(egui::Sense::drag()),
        );

        if resp.hovered() {
            let egui::Pos2 { x, y } = resp.hover_pos().unwrap();
            pointer_ev.send(PointerHits::new(
                PointerId::Mouse,
                vec![(id, HitData::new(id, 0.0, Some(Vec3::new(x, y, 0.0)), None))],
                // egui is at depth 1_000_000, we need to be in front of that.
                1_000_001.0,
            ));
        }
    }
}
