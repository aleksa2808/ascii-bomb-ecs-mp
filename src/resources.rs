use bevy::prelude::*;

pub struct Textures {
    // players + effects
    pub penguin: Handle<ColorMaterial>,
    pub immortal_penguin: Handle<ColorMaterial>,
    pub crook: Handle<ColorMaterial>,
    pub immortal_crook: Handle<ColorMaterial>,
    pub hatter: Handle<ColorMaterial>,
    pub immortal_hatter: Handle<ColorMaterial>,
    pub bat: Handle<ColorMaterial>,
    pub immortal_bat: Handle<ColorMaterial>,
    // bomb + fire
    pub bomb: Handle<ColorMaterial>,
    pub fire: Handle<ColorMaterial>,
    // map tiles
    pub empty: Handle<ColorMaterial>,
    pub wall: Handle<ColorMaterial>,
    pub destructible_wall: Handle<ColorMaterial>,
    pub burning_wall: Handle<ColorMaterial>,
    // items
    pub bombs_up: Handle<ColorMaterial>,
    pub range_up: Handle<ColorMaterial>,
    pub lives_up: Handle<ColorMaterial>,
    pub wall_hack: Handle<ColorMaterial>,
    pub bomb_push: Handle<ColorMaterial>,
    pub immortal: Handle<ColorMaterial>,
    pub burning_item: Handle<ColorMaterial>,
}

pub struct Fonts {
    pub font1: Handle<Font>,
}
