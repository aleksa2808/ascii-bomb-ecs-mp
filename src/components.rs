use bevy::{
    ecs as bevy_ecs, prelude::Component, reflect as bevy_reflect, reflect::Reflect,
    render::color::Color,
};

use crate::types::Direction;

// Lobby

#[derive(Component)]
pub struct LobbyText;
#[derive(Component)]
pub struct LobbyUI;

// HUD display

#[derive(Component, Reflect, Default)]
pub struct UIRoot;

#[derive(Component, Reflect, Default)]
pub struct UIComponent;

#[derive(Component, Reflect, Default)]
pub struct HUDRoot;

#[derive(Component, Reflect, Default)]
pub struct GameTimerDisplay;

#[derive(Component, Reflect, Default)]
pub struct PenguinPortraitDisplay;

#[derive(Component, Reflect, Default, Hash)]
pub struct PenguinPortrait(pub Penguin);

#[derive(Component, Reflect, Default)]
pub struct LeaderboardUI;

// In-game

#[derive(Component, Reflect, Default)]
pub struct Player;

#[derive(Component, Reflect, Default, Hash)]
pub struct Dead {
    pub cleanup_frame: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Component, Reflect, Default)]
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

#[derive(Default, Clone, Reflect, Component, Hash)]
pub struct Bomb {
    pub owner: Option<Penguin>,
    pub range: usize,
    pub expiration_frame: usize,
}

// TODO impl hash?
#[derive(Component, Reflect, Default)]
pub struct Fuse {
    pub color: Color,
    pub start_frame: usize,
}

#[derive(Component, Reflect, Default, Hash)]
pub struct Fire {
    pub expiration_frame: usize,
}

#[derive(Component, Reflect, Default)]
pub struct Solid;

#[derive(Component, Reflect, Default)]
pub struct Wall;

#[derive(Component, Reflect, Default)]
pub struct Destructible;

#[derive(Component, Reflect, Default, Hash)]
pub struct Crumbling {
    pub expiration_frame: usize,
}

#[derive(Component, Reflect, Default, Hash)]
pub struct BombSatchel {
    pub bombs_available: usize,
    pub bomb_range: usize,
}

#[derive(Component, Reflect, Default, Hash)]
pub enum Item {
    #[default]
    BombsUp,
    RangeUp,
    BombPush,
}

#[derive(Component, Reflect, Default, Hash)]
pub struct BurningItem {
    pub expiration_frame: usize,
}
