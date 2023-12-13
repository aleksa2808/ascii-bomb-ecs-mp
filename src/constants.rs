use crate::types::RGBColor;

pub const COLORS: [RGBColor; 16] = [
    RGBColor(12, 12, 12),
    RGBColor(0, 55, 218),
    RGBColor(19, 161, 14),
    RGBColor(58, 150, 221),
    RGBColor(197, 15, 31),
    RGBColor(136, 23, 152),
    RGBColor(193, 156, 0),
    RGBColor(204, 204, 204),
    RGBColor(118, 118, 118),
    RGBColor(59, 120, 255),
    RGBColor(22, 198, 12),
    RGBColor(97, 214, 214),
    RGBColor(231, 72, 86),
    RGBColor(180, 0, 158),
    RGBColor(249, 241, 165),
    RGBColor(242, 242, 242),
];

pub const PIXEL_SCALE: usize = 8;

pub const HUD_HEIGHT: usize = 14 * PIXEL_SCALE;

pub const TILE_HEIGHT: usize = 8 * PIXEL_SCALE;
pub const TILE_WIDTH: usize = 6 * PIXEL_SCALE;

pub const WALL_Z_LAYER: f32 = 60.0;
pub const PLAYER_Z_LAYER: f32 = 50.0;
pub const BOMB_Z_LAYER: f32 = 25.0;
pub const ITEM_Z_LAYER: f32 = 20.0;
pub const DESTRUCTIBLE_WALL_Z_LAYER: f32 = 10.0;
pub const FIRE_Z_LAYER: f32 = 5.0;

pub const INPUT_UP: u8 = 1 << 0;
pub const INPUT_DOWN: u8 = 1 << 1;
pub const INPUT_LEFT: u8 = 1 << 2;
pub const INPUT_RIGHT: u8 = 1 << 3;
pub const INPUT_ACTION: u8 = 1 << 4;

pub const ROUND_DURATION_SECS: usize = 60;

pub const FPS: usize = 30;
pub const MAX_PREDICTED_FRAMES: usize = 8;

// these must not be lower than MAX_PREDICTED_FRAMES
// TODO can some static asserts be made?
pub const GET_READY_DISPLAY_FRAME_COUNT: usize = 3 * FPS;
pub const GAME_START_FREEZE_FRAME_COUNT: usize = FPS / 2;
pub const LEADERBOARD_DISPLAY_FRAME_COUNT: usize = 2 * FPS;
pub const TOURNAMENT_WINNER_DISPLAY_FRAME_COUNT: usize = 5 * FPS;

pub const BOMB_SHORTENED_FUSE_FRAME_COUNT: usize = 2;

pub const MOVING_OBJECT_FRAME_INTERVAL: usize = 1;

// TODO figure out if floats can be used deterministically
pub const ITEM_SPAWN_CHANCE_PERCENTAGE: u64 = 33;
