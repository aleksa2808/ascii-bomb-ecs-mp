use bevy::{prelude::Color, ui::BackgroundColor};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

impl Direction {
    pub const LIST: [Direction; 4] = [
        Direction::Right,
        Direction::Left,
        Direction::Up,
        Direction::Down,
    ];
}
