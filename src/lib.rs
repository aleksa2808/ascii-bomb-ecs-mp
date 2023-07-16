mod components;
mod constants;
mod resources;
mod systems;
mod types;
mod utils;

use bevy::{ecs as bevy_ecs, prelude::*};
use bevy_ggrs::{GgrsAppExtension, GgrsPlugin, GgrsSchedule};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use crate::{
    components::{Bomb, BombSatchel, Crumbling, Fire, Position},
    constants::FPS,
    resources::{Args, Fonts, FrameCount, GameTextures, HUDColors},
    systems::*,
    types::GGRSConfig,
};

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, Hash, States)]
pub enum AppState {
    #[default]
    Lobby,
    InGame,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn run() {
    let args = Args::get();
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

    app.add_ggrs_plugin(
        GgrsPlugin::<GGRSConfig>::new()
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
    .insert_resource(args)
    .insert_resource(FrameCount { frame: 0 })
    .run();

    app.run();
}
