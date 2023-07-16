use std::ffi::OsString;

use bevy::{ecs as bevy_ecs, prelude::*};
use bevy_ggrs::ggrs::PlayerHandle;
use clap::Parser;
use serde::Deserialize;

use crate::{
    constants::{INPUT_ACTION, INPUT_DOWN, INPUT_LEFT, INPUT_RIGHT, INPUT_UP},
    types::PlayerInput,
};

#[derive(Parser, Debug, Clone, Deserialize, Resource)]
#[serde(default)]
#[clap(
    name = "ascii_bomb_ecs_mp",
    rename_all = "kebab-case",
    rename_all_env = "screaming-snake"
)]
pub struct Args {
    #[clap(long, default_value = "wss://match-0-6.helsing.studio")]
    pub signal_server_address: String,

    #[clap(long)]
    pub room: Option<String>,

    #[clap(long, short, default_value = "2")]
    pub number_of_players: usize,
}

impl Default for Args {
    fn default() -> Self {
        let args = Vec::<OsString>::new();
        Args::parse_from(args)
    }
}

impl Args {
    pub fn get() -> Self {
        Args::parse()
    }
}

pub fn native_input(
    _handle: In<PlayerHandle>,
    mut r: Local<u8>,
    keyboard_input: Res<Input<KeyCode>>,
) -> PlayerInput {
    let mut input: u8 = 0;

    if keyboard_input.pressed(KeyCode::Up) {
        input |= INPUT_UP;
    }
    if keyboard_input.pressed(KeyCode::Left) {
        input |= INPUT_LEFT;
    }
    if keyboard_input.pressed(KeyCode::Down) {
        input |= INPUT_DOWN;
    }
    if keyboard_input.pressed(KeyCode::Right) {
        input |= INPUT_RIGHT;
    }
    if keyboard_input.pressed(KeyCode::Space) {
        input |= INPUT_ACTION;
    }

    let inp = !*r & input;
    *r = input;

    PlayerInput { inp }
}
