use bevy::{
    ecs::{component::ComponentId, world::DeferredWorld},
    prelude::*,
};

use super::*;

impl DrawImage {
    pub fn on_insert(_: DeferredWorld, _: Entity, _: ComponentId) {}
}
