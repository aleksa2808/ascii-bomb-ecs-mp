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
    #[clap(long)]
    pub matchbox_server_url: Option<String>,

    #[clap(long, default_value = "quick_join")]
    pub room_id: String,

    #[clap(long, short, default_value = "2")]
    pub number_of_players: u8,
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
    mut last_kb_input: Local<u8>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    // there must be only one local player
    assert_eq!(local_players.0.len(), 1);
    let local_player_handle = *local_players.0.first().unwrap();

    // process keyboard input
    let mut kb_input: u8 = 0;

    if keyboard_input.pressed(KeyCode::Up) {
        kb_input |= INPUT_UP;
    }
    if keyboard_input.pressed(KeyCode::Left) {
        kb_input |= INPUT_LEFT;
    }
    if keyboard_input.pressed(KeyCode::Down) {
        kb_input |= INPUT_DOWN;
    }
    if keyboard_input.pressed(KeyCode::Right) {
        kb_input |= INPUT_RIGHT;
    }
    if keyboard_input.pressed(KeyCode::Space) {
        kb_input |= INPUT_ACTION;
    }

    // only acknowledge new keyboard input
    let input = !*last_kb_input & kb_input;
    *last_kb_input = kb_input;

    let mut local_inputs = HashMap::new();
    if game_freeze.is_some() {
        // override inputs during a freeze as the game must not be rolled back at this time
        local_inputs.insert(local_player_handle, PlayerInput(0));
    } else {
        local_inputs.insert(local_player_handle, PlayerInput(input));
    }

    commands.insert_resource(LocalInputs::<GgrsConfig>(local_inputs));
}
