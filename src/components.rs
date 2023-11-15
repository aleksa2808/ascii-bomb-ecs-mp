use bevy::{
    ecs as bevy_ecs,
    prelude::{Component, Entity},
    reflect as bevy_reflect,
    reflect::Reflect,
    render::color::Color,
};

use crate::types::Direction;

#[derive(Component)]
pub struct LobbyText;
#[derive(Component)]
pub struct LobbyUI;

#[derive(Component)]
pub struct UIRoot;

#[derive(Component)]
pub struct UIComponent;

#[derive(Component)]
pub struct LeaderboardUI;

#[derive(Component)]
pub struct Player;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Component, Reflect)]
pub struct Penguin(pub usize);

#[derive(Default, Reflect, Debug, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub struct Position {
    pub y: isize,
    pub x: isize,
}

impl Position {
    pub fn offset(&self, direction: Direction, distance: usize) -> Self {
        let distance = distance as isize;

        let (y_offset, x_offset) = match direction {
            Direction::Right => (0, distance),
            Direction::Down => (distance, 0),
            Direction::Left => (0, -distance),
            Direction::Up => (-distance, 0),
        };

        Position {
            y: self.y + y_offset,
            x: self.x + x_offset,
        }
    }
}

#[derive(Default, Clone, Reflect, Component)]
pub struct Bomb {
    pub owner: Option<Entity>,
    pub range: usize,
    pub expiration_frame: usize,
}

#[derive(Component)]
pub struct Fuse {
    pub color: Color,
    pub start_frame: usize,
}

#[derive(Default, Reflect, Component)]
pub struct Fire {
    pub expiration_frame: usize,
}

#[derive(Component)]
pub struct Solid;

#[derive(Component)]
pub struct Wall;

#[derive(Component)]
pub struct Destructible;

#[derive(Default, Reflect, Component)]
pub struct Crumbling {
    pub expiration_frame: usize,
}

#[derive(Default, Reflect, Component)]
pub struct BombSatchel {
    pub bombs_available: usize,
    pub bomb_range: usize,
}

// HUD display

#[derive(Component)]
pub struct HUDRoot;

#[derive(Component)]
pub struct GameTimerDisplay;

#[derive(Component)]
pub struct PenguinPortraitDisplay;

#[derive(Component)]
pub struct PenguinPortrait(pub Penguin);
