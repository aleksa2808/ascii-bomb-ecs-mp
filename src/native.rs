use std::ffi::OsString;

use bevy::{ecs as bevy_ecs, prelude::*, utils::HashMap};
use bevy_ggrs::{LocalInputs, LocalPlayers};
use clap::Parser;
use serde::Deserialize;

use crate::{
    constants::{INPUT_ACTION, INPUT_DOWN, INPUT_LEFT, INPUT_RIGHT, INPUT_UP},
    resources::GameFreeze,
    types::{GgrsConfig, PlayerInput},
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
    // #[clap(long, default_value = "ws://127.0.0.1:3536")]
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
    mut commands: Commands,
    keyboard_input: Res<Input<KeyCode>>,
    local_players: Res<LocalPlayers>,
    mut r: Local<Vec<u8>>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if r.len() != local_players.0.len() {
        *r = vec![0; local_players.0.len()];
    }

    let mut local_inputs = HashMap::new();

    for (i, handle) in local_players.0.iter().enumerate() {
        if game_freeze.is_some() {
            // The game should not be rollbacked during a freeze.
            local_inputs.insert(*handle, PlayerInput(0));
        } else {
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

            let inp = !r[i] & input;
            r[i] = input;

            local_inputs.insert(*handle, PlayerInput(inp));
        }
    }

    commands.insert_resource(LocalInputs::<GgrsConfig>(local_inputs));
}
