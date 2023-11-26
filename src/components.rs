use bevy::{ecs as bevy_ecs, prelude::Component, render::color::Color};

use crate::types::{Direction, PlayerID};

#[derive(Component)]
pub struct FullscreenMessageText;

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
pub struct PlayerPortraitDisplay;

#[derive(Component)]
pub struct PlayerPortrait(pub PlayerID);

#[derive(Component)]
pub struct LeaderboardUIRoot;

#[derive(Component)]
pub struct LeaderboardUIContent;

// In-game

#[derive(Component, Clone, Copy, Hash)]
pub struct Player {
    pub id: PlayerID,
    pub can_push_bombs: bool,
}

#[derive(Component, Clone, Copy)]
pub struct Dead {
    pub cleanup_frame: usize,
}

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

#[derive(Component, Clone, Copy, Hash)]
pub struct BombSatchel {
    pub bombs_available: usize,
    pub bomb_range: usize,
}

#[derive(Component, Clone, Copy)]
pub struct Bomb {
    pub owner: Option<PlayerID>,
    pub range: usize,
    pub expiration_frame: usize,
}

#[derive(Component, Clone, Copy)]
pub struct Moving {
    pub direction: Direction,
    pub next_move_frame: usize,
    pub frame_interval: usize,
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

#[derive(Component, Debug, Clone, Copy, Hash)]
pub enum Item {
    BombsUp,
    RangeUp,
    BombPush,
}

#[derive(Component, Clone, Copy)]
pub struct BurningItem {
    pub expiration_frame: usize,
}
