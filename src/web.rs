use std::collections::VecDeque;

use bevy::prelude::*;
use bevy_ggrs::ggrs::PlayerHandle;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::{
    constants::{INPUT_ACTION, INPUT_DOWN, INPUT_LEFT, INPUT_RIGHT, INPUT_UP},
    resources::MatchboxConfig,
    types::PlayerInput,
    AppState,
};

static START: Lazy<RwLock<Option<(String, usize)>>> = Lazy::new(|| RwLock::new(None));
static INPUTS: Lazy<RwLock<VecDeque<u8>>> = Lazy::new(|| RwLock::new(VecDeque::new()));

// functions callable from JavaScript
#[wasm_bindgen]
#[allow(dead_code)]
pub fn start_game(signal_server_address: &str) {
    let number_of_players = 2;
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
    W,
    S,
    A,
    D,
    G,
    Return,
    Escape,
    Back,
    F,
}

pub fn web_input(
    _handle: In<PlayerHandle>,
    mut r: Local<u8>,
    keyboard_input: Res<Input<KeyCode>>,
) -> PlayerInput {
    let mut web_input: u8 = 0;

    let mut inputs = INPUTS.write();
    while let Some(input) = inputs.pop_back() {
        if let Some(input_action) = match input {
            0 => Some(InputAction::Up),
            1 => Some(InputAction::Down),
            2 => Some(InputAction::Left),
            3 => Some(InputAction::Right),
            4 => Some(InputAction::Space),
            5 => Some(InputAction::W),
            6 => Some(InputAction::S),
            7 => Some(InputAction::A),
            8 => Some(InputAction::D),
            9 => Some(InputAction::G),
            10 => Some(InputAction::Return),
            11 => Some(InputAction::Escape),
            12 => Some(InputAction::Back),
            13 => Some(InputAction::F),
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
                _ => (),
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

    let inp = !*r & kb_input | web_input;
    *r = kb_input;

    PlayerInput { inp }
}
