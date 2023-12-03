use std::collections::VecDeque;

use bevy::{prelude::*, utils::HashMap};
use bevy_ggrs::{LocalInputs, LocalPlayers};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::{
    constants::{INPUT_ACTION, INPUT_DOWN, INPUT_LEFT, INPUT_RIGHT, INPUT_UP},
    resources::{GameFreeze, MatchboxConfig},
    types::{GgrsConfig, PlayerInput},
    AppState,
};

static START: Lazy<RwLock<Option<(String, usize)>>> = Lazy::new(|| RwLock::new(None));
static INPUTS: Lazy<RwLock<VecDeque<u8>>> = Lazy::new(|| RwLock::new(VecDeque::new()));

// functions callable from JavaScript
#[wasm_bindgen]
#[allow(dead_code)]
pub fn start_game(signal_server_address: &str, number_of_players: usize) {
    info!("start_game: {signal_server_address} {number_of_players}");
    let mut start = START.write();
    *start = Some((signal_server_address.to_string(), number_of_players));
}

#[wasm_bindgen]
#[allow(dead_code)]
pub fn set_input_active(input: u8) {
    let mut inputs = INPUTS.write();
    inputs.push_front(input);
}

pub fn web_ready_to_start_update(
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if let Some((signal_server_address, number_of_players)) = START.read().clone() {
        commands.insert_resource(MatchboxConfig {
            signal_server_address,
            room: None,
            number_of_players,
        });
        next_state.set(AppState::Lobby);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputAction {
    Up,
    Down,
    Left,
    Right,
    Space,
}

pub fn web_input(
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
            let mut web_input: u8 = 0;

            let mut inputs = INPUTS.write();
            while let Some(input) = inputs.pop_back() {
                if let Some(input_action) = match input {
                    0 => Some(InputAction::Up),
                    1 => Some(InputAction::Down),
                    2 => Some(InputAction::Left),
                    3 => Some(InputAction::Right),
                    4 => Some(InputAction::Space),
                    _ => None,
                } {
                    match input_action {
                        InputAction::Up => {
                            web_input |= INPUT_UP;
                        }
                        InputAction::Down => {
                            web_input |= INPUT_DOWN;
                        }
                        InputAction::Left => {
                            web_input |= INPUT_LEFT;
                        }
                        InputAction::Right => {
                            web_input |= INPUT_RIGHT;
                        }
                        InputAction::Space => {
                            web_input |= INPUT_ACTION;
                        }
                    }
                }
            }

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

            let inp = !r[i] & kb_input | web_input;
            r[i] = kb_input;

            local_inputs.insert(*handle, PlayerInput(inp));
        }
    }

    commands.insert_resource(LocalInputs::<GgrsConfig>(local_inputs));
}
