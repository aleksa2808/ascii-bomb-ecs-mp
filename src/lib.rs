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
use crate::{components::*, constants::FPS, resources::*, systems::*, types::GGRSConfig};
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
            // Bevy components
            .register_rollback_component::<Sprite>()
            .register_rollback_component::<Transform>()
            .register_rollback_component::<GlobalTransform>()
            .register_rollback_component::<Handle<Image>>()
            .register_rollback_component::<Visibility>()
            .register_rollback_component::<ComputedVisibility>()
            // HUD components
            // TODO not sure if these are necessary
            .register_rollback_component::<UIRoot>()
            .register_rollback_component::<UIComponent>()
            .register_rollback_component::<HUDRoot>()
            .register_rollback_component::<GameTimerDisplay>()
            .register_rollback_component::<PenguinPortraitDisplay>()
            .register_rollback_component::<PenguinPortrait>()
            .register_rollback_component::<LeaderboardUI>()
            // game components
            .register_rollback_component::<Player>()
            .register_rollback_component::<Dead>()
            .register_rollback_component::<Penguin>()
            .register_rollback_component::<Position>()
            .register_rollback_component::<Bomb>()
            .register_rollback_component::<Fuse>()
            .register_rollback_component::<Fire>()
            .register_rollback_component::<Solid>()
            .register_rollback_component::<Wall>()
            .register_rollback_component::<Destructible>()
            .register_rollback_component::<Crumbling>()
            .register_rollback_component::<BombSatchel>()
            .register_rollback_component::<Item>()
            .register_rollback_component::<BurningItem>()
            // resources
            .register_rollback_resource::<FrameCount>()
            // TODO not sure if this is necessary
            // .register_rollback_resource::<Leaderboard>()
            .register_rollback_resource::<RoundOutcome>()
            .register_rollback_resource::<GameEndFrame>()
            .register_rollback_resource::<FreezeEndFrame>()
            // TODO not sure if this is necessary
            .register_rollback_resource::<TournamentComplete>(),
    )
    .add_systems(
        GgrsSchedule,
        // list too long for one chain
        // TODO prune apply_deferred calls
        (
            (
                increase_frame_system,
                show_leaderboard,
                start_new_round,
                start_new_tournament,
                update_hud_clock,
                update_player_portraits,
                apply_deferred,
                player_move,
                bomb_drop,
                apply_deferred,
            )
                .chain(),
            (
                fire_tick,
                apply_deferred,
                crumbling_tick,
                apply_deferred,
                burning_item_tick,
                apply_deferred,
                explode_bombs,
                apply_deferred,
                animate_fuse,
                player_burn,
                apply_deferred,
                item_burn,
                apply_deferred,
                finish_round,
                apply_deferred,
                cleanup_dead,
            )
                .chain(),
        )
            .chain(),
    )
    .insert_resource(FrameCount { frame: 0 })
    .run();

    app.run();
}
