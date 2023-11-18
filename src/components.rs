use bevy::{ecs as bevy_ecs, prelude::Component, render::color::Color};

use crate::types::Direction;

// Lobby

#[derive(Component)]
pub struct LobbyText;
#[derive(Component)]
pub struct LobbyUI;

// HUD display

#[derive(Component)]
pub struct UIRoot;

#[derive(Component)]
pub struct UIComponent;

#[derive(Component)]
pub struct HUDRoot;

#[derive(Component)]
pub struct GameTimerDisplay;

#[derive(Component)]
pub struct PenguinPortraitDisplay;

#[derive(Component)]
pub struct PenguinPortrait(pub Penguin);

#[derive(Component)]
pub struct LeaderboardUI;

// In-game

#[derive(Component, Clone, Copy)]
pub struct Player;

#[derive(Component, Clone, Copy)]
pub struct Dead {
    pub cleanup_frame: usize,
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Penguin(pub usize);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

#[derive(Component, Clone, Copy)]
pub struct Bomb {
    pub owner: Option<Penguin>,
    pub range: usize,
    pub expiration_frame: usize,
}

#[derive(Component, Clone, Copy)]
pub struct Fuse {
    pub color: Color,
    pub start_frame: usize,
}

#[derive(Component, Clone, Copy)]
pub struct Fire {
    pub expiration_frame: usize,
}

#[derive(Component, Clone, Copy)]
pub struct Solid;

#[derive(Component, Clone, Copy)]
pub struct Wall;

#[derive(Component, Clone, Copy)]
pub struct Destructible;

#[derive(Component, Clone, Copy)]
pub struct Crumbling {
    pub expiration_frame: usize,
}

#[derive(Component, Clone, Copy, Hash)]
pub struct BombSatchel {
    pub bombs_available: usize,
    pub bomb_range: usize,
}

#[derive(Component, Clone, Copy, Hash)]
pub enum Item {
    BombsUp,
    RangeUp,
    BombPush,
}

#[derive(Component, Clone, Copy)]
pub struct BurningItem {
    pub expiration_frame: usize,
}
