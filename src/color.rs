use bevy::prelude::*;
use bevy_trait_query::RegisterExt;
use bevy_vector_shapes::prelude::*;

#[bevy_trait_query::queryable]
pub trait HasColor {
    fn color_mut(&mut self) -> &mut Color;
}

macro_rules! has_color {
    ($entry:ident) => {
        $entry!(Sprite);
        $entry!(Rectangle);
        $entry!(Disc);
        $entry!(Line);
        $entry!(Triangle);
        $entry!(RegularPolygon);
    };
    ($entry:ident, $arg:tt) => {
        $entry!(Sprite, $arg);
        $entry!(Rectangle, $arg);
        $entry!(Disc, $arg);
        $entry!(Line, $arg);
        $entry!(Triangle, $arg);
        $entry!(RegularPolygon, $arg);
    };
}

macro_rules! impl_has_color {
    ($ty:ident) => {
        impl HasColor for $ty {
            fn color_mut(&mut self) -> &mut Color {
                &mut self.color
            }
        }
    };
}

has_color!(impl_has_color);

macro_rules! register_has_color {
    ($ty:ident, $app:expr) => {
        $app.register_component_as::<dyn HasColor, $ty>()
    };
}

pub struct ColorPlugin;

impl Plugin for ColorPlugin {
    fn build(&self, app: &mut App) {
        has_color!(register_has_color, app);
    }
}

pub fn plugin() -> ColorPlugin {
    ColorPlugin
}
