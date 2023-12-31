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
pub struct NetworkStatsDisplay;

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
    pub cleanup_frame: u32,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    pub y: u8,
    pub x: u8,
}

impl Position {
    pub fn offset(&self, direction: Direction, distance: u8) -> Self {
        let (new_y, new_x) = match direction {
            Direction::Right => (self.y, self.x + distance),
            Direction::Down => (self.y + distance, self.x),
            Direction::Left => (self.y, self.x - distance),
            Direction::Up => (self.y - distance, self.x),
        };

        Position { y: new_y, x: new_x }
    }
}

#[derive(Component, Clone, Copy, Hash)]
pub struct BombSatchel {
    pub bombs_available: u8,
    pub bomb_range: u8,
}

#[derive(Component, Clone, Copy)]
pub struct Bomb {
    pub owner: Option<PlayerID>,
    pub range: u8,
    pub expiration_frame: u32,
}

#[derive(Component, Clone, Copy)]
pub struct Moving {
    pub direction: Direction,
    pub next_move_frame: u32,
    pub frame_interval: u32,
}

#[derive(Component, Clone, Copy)]
pub struct Fuse {
    pub color: Color,
    pub start_frame: u32,
}

#[derive(Component, Clone, Copy)]
pub struct Fire {
    pub expiration_frame: u32,
}

#[derive(Component, Clone, Copy)]
pub struct Solid;

#[derive(Component, Clone, Copy)]
pub struct Wall;

#[derive(Component, Clone, Copy)]
pub struct Destructible;

#[derive(Component, Clone, Copy)]
pub struct Crumbling {
    pub expiration_frame: u32,
}

#[derive(Component, Debug, Clone, Copy)]
pub enum Item {
    BombsUp,
    RangeUp,
    BombPush,
}

#[derive(Component, Clone, Copy)]
pub struct BurningItem {
    pub expiration_frame: u32,
}
