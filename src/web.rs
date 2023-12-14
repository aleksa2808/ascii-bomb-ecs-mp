use std::collections::VecDeque;

use bevy::{prelude::*, utils::HashMap};
use bevy_ggrs::{LocalInputs, LocalPlayers};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::{
    constants::{INPUT_ACTION, INPUT_DOWN, INPUT_LEFT, INPUT_RIGHT, INPUT_UP},
    resources::{GameFreeze, MatchboxConfig},
    types::{GgrsConfig, ICEServerConfig, PlayerInput},
    AppState,
};

static START: Lazy<RwLock<Option<(u8, String, String, String, String, String)>>> =
    Lazy::new(|| RwLock::new(None));
static INPUTS: Lazy<RwLock<VecDeque<u8>>> = Lazy::new(|| RwLock::new(VecDeque::new()));

// functions callable from JavaScript
#[wasm_bindgen]
#[allow(dead_code)]
pub fn start_game(
    number_of_players: u8,
    room_id: &str,
    matchbox_server_url: &str,
    ice_server_url: &str,
    turn_server_username: &str,
    turn_server_credential: &str,
) {
    info!("start_game configs:");
    info!("player count: {number_of_players}");
    info!("room id: {room_id}");
    info!("matchbox server url: {matchbox_server_url}");
    info!("stun/turn server url: {ice_server_url}");
    info!("turn server username: {turn_server_username}");
    info!("turn server credential: {turn_server_credential}");
    let mut start = START.write();
    *start = Some((
        number_of_players,
        room_id.to_string(),
        matchbox_server_url.to_string(),
        ice_server_url.to_string(),
        turn_server_username.to_string(),
        turn_server_credential.to_string(),
    ));
}

#[wasm_bindgen]
#[allow(dead_code)]
pub fn set_input_active(input: u8) {
    let mut inputs = INPUTS.write();
    inputs.push_front(input);
}

// callable JavaScript functions
#[wasm_bindgen(module = "/src/wasm_callables.js")]
extern "C" {
    pub fn doneLoading();
}

// web-specific systems
pub fn web_ready_to_start_enter() {
    // TODO: would it be better to do this through web-sys?
    doneLoading();
}

pub fn web_ready_to_start_update(
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if let Some((
        number_of_players,
        room_id,
        matchbox_server_url,
        ice_server_url,
        turn_server_username,
        turn_server_credential,
    )) = START.read().clone()
    {
        let matchbox_server_url = if !matchbox_server_url.trim().is_empty() {
            Some(matchbox_server_url)
        } else {
            None
        };

        let ice_server_config = if !ice_server_url.trim().is_empty() {
            let username = if !turn_server_username.trim().is_empty() {
                Some(turn_server_username)
            } else {
                None
            };
            let credential = if !turn_server_credential.trim().is_empty() {
                Some(turn_server_credential)
            } else {
                None
            };

            Some(ICEServerConfig {
                url: ice_server_url,
                username,
                credential,
            })
        } else {
            None
        };

        commands.insert_resource(MatchboxConfig {
            number_of_players,
            room_id,
            matchbox_server_url,
            ice_server_config,
        });
        next_state.set(AppState::Lobby);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
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
    mut last_kb_input: Local<u8>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    // there must be only one local player
    assert_eq!(local_players.0.len(), 1);
    let local_player_handle = *local_players.0.first().unwrap();

    // process web UI input
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

    // merge the inputs while only acknowledging new keyboard input
    let input = !*last_kb_input & kb_input | web_input;
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
