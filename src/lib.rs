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
use bevy_ggrs::prelude::*;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
use crate::web::{web_input, web_ready_to_start_update};
use crate::{components::*, constants::FPS, resources::*, systems::*, types::GgrsConfig};
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
    Invalid,
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
        (setup_lobby, start_matchbox_socket),
    )
    .add_systems(Update, lobby_system.run_if(in_state(AppState::Lobby)))
    .add_systems(OnExit(AppState::Lobby), teardown_lobby)
    .add_systems(OnEnter(AppState::InGame), setup_game)
    .add_systems(
        Update,
        handle_ggrs_events.run_if(in_state(AppState::InGame)),
    );

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

    app.add_plugins(GgrsPlugin::<GgrsConfig>::default())
        .set_rollback_schedule_fps(FPS)
        .add_systems(ReadInputs, input_fn)
        // Bevy components
        .rollback_component_with_clone::<Sprite>()
        .rollback_component_with_copy::<Transform>()
        .rollback_component_with_copy::<GlobalTransform>()
        .rollback_component_with_clone::<Handle<Image>>()
        .rollback_component_with_copy::<Visibility>()
        .rollback_component_with_copy::<InheritedVisibility>()
        .rollback_component_with_copy::<ViewVisibility>()
        // game components
        .rollback_component_with_copy::<Player>()
        .rollback_component_with_copy::<Dead>()
        .rollback_component_with_copy::<Position>()
        .rollback_component_with_copy::<Bomb>()
        .rollback_component_with_copy::<Moving>()
        .rollback_component_with_copy::<Fuse>()
        .rollback_component_with_copy::<Fire>()
        .rollback_component_with_copy::<Solid>()
        .rollback_component_with_copy::<Wall>()
        .rollback_component_with_copy::<Destructible>()
        .rollback_component_with_copy::<Crumbling>()
        .rollback_component_with_copy::<BombSatchel>()
        .rollback_component_with_copy::<Item>()
        .rollback_component_with_copy::<BurningItem>()
        // resources
        .rollback_resource_with_clone::<SessionRng>()
        .rollback_resource_with_copy::<FrameCount>()
        .rollback_resource_with_copy::<WallOfDeath>()
        // TODO not sure if this is necessary
        .rollback_resource_with_copy::<GameFreeze>()
        // TODO what if two items are switched, is their order also hashed?
        .checksum_component_with_hash::<Player>()
        .checksum_component_with_hash::<Position>()
        .checksum_component_with_hash::<BombSatchel>()
        .checksum_component_with_hash::<Item>()
        // .checksum_resource_with_hash::<SessionRng>()
        .add_systems(
            GgrsSchedule,
            // list too long for one chain
            // TODO prune apply_deferred calls
            (
                (
                    increase_frame_system,
                    show_leaderboard,
                    apply_deferred,
                    show_tournament_winner,
                    apply_deferred,
                    start_new_round,
                    apply_deferred,
                    finish_actionless_game_freeze,
                    apply_deferred,
                    update_hud_clock,
                    update_player_portraits,
                    apply_deferred,
                    player_move,
                    apply_deferred,
                    moving_object_update,
                    apply_deferred,
                    pick_up_item,
                    apply_deferred,
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
                    bomb_burn,
                    apply_deferred,
                    item_burn,
                    apply_deferred,
                    wall_of_death_update,
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
        // TODO add network stats
        // .insert_resource(NetworkStatsTimer(Timer::from_seconds(
        //     2.0,
        //     TimerMode::Repeating,
        // )))
        // .add_systems(Update, print_network_stats_system)
        .run();
}
