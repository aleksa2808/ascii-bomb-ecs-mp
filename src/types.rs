use bevy::{prelude::Color, ui::BackgroundColor};
use bevy_ggrs::ggrs::Config;
use bevy_matchbox::prelude::PeerId;
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Pod, Zeroable)]
pub struct PlayerInput(pub u8);

#[derive(Debug)]
pub struct GgrsConfig;

impl Config for GgrsConfig {
    type Input = PlayerInput;
    type State = u8;
    type Address = PeerId;
}

pub struct ICEServerConfig {
    pub url: String,
    pub username: Option<String>,
    pub credential: Option<String>,
}

#[derive(Clone, Copy)]
pub struct RGBColor(pub u8, pub u8, pub u8);

impl From<RGBColor> for Color {
    fn from(color: RGBColor) -> Self {
        Self::rgb_u8(color.0, color.1, color.2)
    }
}

impl From<RGBColor> for BackgroundColor {
    fn from(color: RGBColor) -> Self {
        Self(color.into())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerID(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

impl Direction {
    pub const LIST: [Self; 4] = [Self::Right, Self::Left, Self::Up, Self::Down];
}

#[derive(Clone, Copy, Hash)]
pub enum RoundOutcome {
    Tie,
    Winner(PlayerID),
}

#[derive(Clone, Copy)]
pub enum PostFreezeAction {
    ShowLeaderboard(RoundOutcome),
    ShowTournamentWinner { winner: PlayerID },
    StartNewRound,
}
