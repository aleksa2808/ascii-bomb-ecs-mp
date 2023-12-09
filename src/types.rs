use std::time::Duration;

use bevy::{
    prelude::Color,
    time::{Timer, TimerMode},
    ui::BackgroundColor,
};
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

pub enum CooldownState {
    Ready,
    CoolingDown(Timer),
}

pub struct Cooldown {
    state: CooldownState,
    duration: Duration,
}

impl Cooldown {
    pub fn from_seconds(duration: f32) -> Self {
        Cooldown {
            state: CooldownState::Ready,
            duration: Duration::from_secs_f32(duration),
        }
    }

    pub fn trigger(&mut self) -> bool {
        if matches!(self.state, CooldownState::Ready) {
            self.state = CooldownState::CoolingDown(Timer::from_seconds(
                self.duration.as_secs_f32(),
                TimerMode::Once,
            ));
            true
        } else {
            false
        }
    }

    pub fn tick(&mut self, delta: Duration) {
        match self.state {
            CooldownState::Ready => (),
            CooldownState::CoolingDown(ref mut timer) => {
                timer.tick(delta);
                if timer.finished() {
                    self.state = CooldownState::Ready;
                }
            }
        };
    }

    pub fn reset(&mut self) {
        self.state = CooldownState::Ready;
    }

    #[allow(dead_code)]
    pub fn duration(&self) -> Duration {
        self.duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cooldown() {
        let mut cooldown = Cooldown::from_seconds(0.5);
        assert_eq!(cooldown.duration(), Duration::from_millis(500));

        assert!(cooldown.trigger());
        assert!(!cooldown.trigger());

        cooldown.tick(Duration::from_secs_f32(0.3));
        assert!(!cooldown.trigger());

        cooldown.tick(Duration::from_secs_f32(0.2));
        assert!(cooldown.trigger());
        assert!(!cooldown.trigger());

        cooldown.reset();
        assert!(cooldown.trigger());
        assert!(!cooldown.trigger());
    }
}
