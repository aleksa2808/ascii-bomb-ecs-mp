mod components;
mod constants;
#[cfg(not(target_arch = "wasm32"))]
mod native;
mod resources;
mod systems;
mod types;
mod utils;
#[cfg(target_arch = "wasm32")]
mod web;

use bevy::{ecs as bevy_ecs, prelude::*};
use bevy_ggrs::{GgrsAppExtension, GgrsPlugin, GgrsSchedule};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
use crate::web::{web_input, web_ready_to_start_update};
use crate::{
    components::{BombSatchel, Position},
    constants::FPS,
    resources::{Fonts, FrameCount, FreezeEndFrame, GameTextures, HUDColors, RoundOutcome},
    systems::*,
    types::GGRSConfig,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::{
    native::{native_input, Args},
    resources::MatchboxConfig,
};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, States)]
pub enum AppState {
    #[cfg(target_arch = "wasm32")]
    WebReadyToStart,
    Lobby,
    InGame,
}

impl Default for AppState {
    fn default() -> Self {
        cfg_if::cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                Self::WebReadyToStart
            } else {
                Self::Lobby
            }
        }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn run() {
    #[cfg(not(target_arch = "wasm32"))]
    let args = Args::get();
    #[cfg(not(target_arch = "wasm32"))]
    info!("{args:?}");

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "ascii-bomb-ecs".to_string(),
                    resizable: false,
                    #[cfg(target_arch = "wasm32")]
                    canvas: Some("#bevy-canvas".to_string()),
                    ..Default::default()
                }),
                ..default()
            })
            // fixes blurry textures
            .set(ImagePlugin::default_nearest()),
    )
    .init_resource::<Fonts>()
    .init_resource::<HUDColors>()
    .init_resource::<GameTextures>()
    .add_state::<AppState>()
    .add_systems(
        OnEnter(AppState::Lobby),
        (lobby_startup, start_matchbox_socket),
    )
    .add_systems(Update, lobby_system.run_if(in_state(AppState::Lobby)))
    .add_systems(OnExit(AppState::Lobby), lobby_cleanup)
    .add_systems(OnEnter(AppState::InGame), setup_battle_mode)
    .add_systems(Update, log_ggrs_events.run_if(in_state(AppState::InGame)));

    #[cfg(not(target_arch = "wasm32"))]
    app.insert_resource(MatchboxConfig {
        signal_server_address: args.signal_server_address,
        room: args.room,
        number_of_players: args.number_of_players,
    });

    #[cfg(target_arch = "wasm32")]
    app.add_systems(
        Update,
        web_ready_to_start_update.run_if(in_state(AppState::WebReadyToStart)),
    );

    #[cfg(target_arch = "wasm32")]
    let input_fn = web_input;
    #[cfg(not(target_arch = "wasm32"))]
    let input_fn = native_input;

    app.add_ggrs_plugin(
        GgrsPlugin::<GGRSConfig>::new()
            .with_update_frequency(FPS)
            .with_input_system(input_fn)
            .register_rollback_component::<Transform>()
            .register_rollback_component::<Position>()
            // .register_rollback_component::<Bomb>()
            .register_rollback_component::<BombSatchel>()
            // .register_rollback_component::<Fire>()
            // .register_rollback_component::<Crumbling>()
            .register_rollback_resource::<FrameCount>()
            .register_rollback_resource::<RoundOutcome>()
            // .register_rollback_resource::<more?>()
            .register_rollback_resource::<FreezeEndFrame>(),
    )
    .add_systems(
        GgrsSchedule,
        (
            increase_frame_system,
            show_leaderboard,
            start_new_round,
            start_new_tournament,
            apply_deferred,
            player_move,
            bomb_drop,
            apply_deferred,
            fire_tick,
            apply_deferred,
            crumbling_tick,
            apply_deferred,
            explode_bombs,
            apply_deferred,
            animate_fuse,
            player_burn,
            apply_deferred,
            finish_round,
        )
            .chain(),
    )
    .insert_resource(FrameCount { frame: 0 })
    .run();

    app.run();
}
