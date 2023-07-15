use std::net::SocketAddr;

use bevy::{
    core::{Pod, Zeroable},
    prelude::*,
};
use bevy_ggrs::{
    ggrs::{Config, PlayerType, SessionBuilder, UdpNonBlockingSocket},
    GgrsAppExtension, GgrsPlugin, GgrsSchedule, Session,
};
use structopt::StructOpt;

mod components;
mod constants;
mod resources;
mod systems;
mod types;
mod utils;

use systems::*;

use crate::{
    components::{Bomb, BombSatchel, Crumbling, Fire, Position},
    constants::FPS,
    resources::{Fonts, FrameCount, GameTextures, HUDColors, Opt},
};

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Pod, Zeroable)]
pub struct PlayerInput {
    pub inp: u8,
}

#[derive(Debug)]
pub struct GgrsConfig;
impl Config for GgrsConfig {
    type Input = PlayerInput;
    type State = u8;
    type Address = SocketAddr;
}

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, Hash, States)]
pub enum AppState {
    #[default]
    Lobby,
    InGame,
}

pub fn main() {
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
    .add_state::<AppState>();

    app.init_resource::<Fonts>()
        .init_resource::<HUDColors>()
        .init_resource::<GameTextures>();

    // read cmd line arguments
    let opt = Opt::from_args();
    let num_players = opt.players.len();
    assert!(num_players > 0);

    let mut sess_build = SessionBuilder::<GgrsConfig>::new()
        .with_num_players(2)
        .with_desync_detection_mode(bevy_ggrs::ggrs::DesyncDetection::On { interval: 10 }) // (optional) set how often to exchange state checksums
        .with_max_prediction_window(12)
        .with_input_delay(2);

    for (i, player_addr) in opt.players.iter().enumerate() {
        if player_addr == "localhost" {
            sess_build = sess_build.add_player(PlayerType::Local, i).unwrap();
        } else {
            let remote_addr: SocketAddr = player_addr.parse().unwrap();
            sess_build = sess_build
                .add_player(PlayerType::Remote(remote_addr), i)
                .unwrap();
        }
    }

    for (i, spec_addr) in opt.spectators.iter().enumerate() {
        sess_build = sess_build
            .add_player(PlayerType::Spectator(*spec_addr), num_players + i)
            .unwrap();
    }

    let socket = UdpNonBlockingSocket::bind_to_port(opt.local_port).unwrap();
    let sess = sess_build.start_p2p_session(socket).unwrap();

    app.add_ggrs_plugin(
        GgrsPlugin::<GgrsConfig>::new()
            .with_update_frequency(FPS)
            .with_input_system(input)
            .register_rollback_component::<Transform>()
            .register_rollback_component::<Position>()
            .register_rollback_component::<Bomb>()
            .register_rollback_component::<BombSatchel>()
            .register_rollback_component::<Fire>()
            .register_rollback_component::<Crumbling>()
            .register_rollback_resource::<FrameCount>(),
    )
    .insert_resource(opt)
    .add_systems(Startup, setup_battle_mode)
    .add_systems(
        GgrsSchedule,
        (
            increase_frame_system,
            player_move,
            bomb_drop,
            fire_tick,
            crumbling_tick,
            explode_bombs,
        )
            .chain(),
    )
    .insert_resource(Session::P2P(sess))
    .insert_resource(FrameCount { frame: 0 })
    .run();

    app.run();
}
