mod audio;
mod battle_mode;
mod common;
mod game;
mod loading;
mod main_menu;
mod map_transition;
mod secret_mode;
mod splash_screen;
mod story_mode;
#[cfg(target_arch = "wasm32")]
mod web;

use std::{net::SocketAddr, time::Duration};

use audio::Audio;
use battle_mode::BattleModeConfiguration;
use bevy::{
    core::{Pod, Zeroable},
    ecs as bevy_ecs,
    prelude::*,
    reflect as bevy_reflect,
    utils::{HashMap, HashSet},
    window::{PrimaryWindow, WindowResolution},
};
use bevy_ggrs::{
    ggrs::{Config, PlayerHandle, PlayerType, SessionBuilder, UdpNonBlockingSocket},
    AddRollbackCommandExtension, GgrsAppExtension, GgrsPlugin, GgrsSchedule, PlayerInputs,
    Rollback, Session,
};
use common::resources::Fonts;
use game::{
    components::{
        BaseTexture, BombPush, BombSatchel, BotAI, BurningItem, Destructible, Health,
        HumanControlled, ImmortalTexture, MoveCooldown, Penguin, Player, Position, Solid,
        SpawnPosition, TeamID, UIComponent, UIRoot, Wall, WallHack,
    },
    constants::{HUD_HEIGHT, TILE_HEIGHT, TILE_WIDTH},
    resources::{GameTextures, GameTimer, HUDColors, MapSize, Sounds, WorldID},
    types::{BotDifficulty, Cooldown, PlayerAction},
    utils::{get_x, get_y, init_hud, spawn_map},
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use structopt::StructOpt;

use crate::{
    audio::AudioPlugin,
    battle_mode::BattleModePlugin,
    common::{
        constants::{COLORS, PIXEL_SCALE},
        CommonPlugin,
    },
    game::{components::Fuse, GamePlugin},
};
#[cfg(target_arch = "wasm32")]
use crate::{loading::LoadingPlugin, web::*};

const FPS: usize = 60;

const INPUT_UP: u8 = 1 << 0;
const INPUT_DOWN: u8 = 1 << 1;
const INPUT_LEFT: u8 = 1 << 2;
const INPUT_RIGHT: u8 = 1 << 3;
const INPUT_ACTION: u8 = 1 << 4;

// structopt will read command line parameters for u
#[derive(StructOpt, Resource)]
struct Opt {
    #[structopt(short, long)]
    local_port: u16,
    #[structopt(short, long)]
    players: Vec<String>,
    #[structopt(short, long)]
    spectators: Vec<SocketAddr>,
}

#[derive(Debug)]
pub struct GgrsConfig;
impl Config for GgrsConfig {
    type Input = PlayerInput;
    type State = u8;
    type Address = SocketAddr;
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Pod, Zeroable)]
pub struct PlayerInput {
    pub inp: u8,
}

#[derive(Default, Clone, Reflect, Component)]
pub struct Bomb {
    pub owner: Option<Entity>,
    pub range: usize,
    pub expiration_frame: usize,
}

#[derive(Default, Reflect, Component)]
pub struct Fire {
    pub expiration_frame: usize,
}

#[derive(Default, Reflect, Component)]
pub struct Crumbling {
    pub expiration_frame: usize,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, States)]
pub enum AppState {
    #[cfg(target_arch = "wasm32")]
    Loading,
    #[cfg(target_arch = "wasm32")]
    WebReadyToStart,
    SplashScreen,
    MainMenu,
    MapTransition,
    StoryModeSetup,
    StoryModeManager,
    BossSpeech,
    StoryModeInGame,
    HighScoreNameInput,
    StoryModeTeardown,
    BattleModeSetup,
    BattleModeManager,
    RoundStartFreeze,
    BattleModeInGame,
    LeaderboardDisplay,
    BattleModeTeardown,
    Paused,
    SecretModeSetup,
    SecretModeManager,
    SecretModeInGame,
    SecretModeTeardown,
}

impl Default for AppState {
    fn default() -> Self {
        cfg_if::cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                Self::Loading
            } else {
                // The loading state is not used in the native build in order to mimic
                // the original game's behavior (non-blocking splash screen)
                Self::BattleModeSetup
            }
        }
    }
}

#[derive(Resource, Default, Reflect, Hash)]
#[reflect(Hash)]
pub struct FrameCount {
    pub frame: usize,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn run() {
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
    // .add_state::<AppState>()
    .add_plugins(AudioPlugin);

    #[cfg(target_arch = "wasm32")]
    app.add_plugins(LoadingPlugin {
        loading_state: AppState::Loading,
        next_state: AppState::WebReadyToStart,
    })
    .add_systems(
        Update,
        handle_web_input.in_set(crate::common::Label::InputMapping),
    )
    .add_systems(OnEnter(AppState::WebReadyToStart), web_ready_to_start_enter)
    .add_systems(
        Update,
        web_ready_to_start_update.run_if(in_state(AppState::WebReadyToStart)),
    );

    app.init_resource::<Fonts>()
        .init_resource::<HUDColors>()
        .init_resource::<GameTextures>();

    app.insert_resource(BattleModeConfiguration {
        amount_of_players: 4,
        amount_of_bots: 0,
        winning_score: 1,
        bot_difficulty: BotDifficulty::Medium,
    });

    // read cmd line arguments
    let opt = Opt::from_args();
    let num_players = opt.players.len();
    assert!(num_players > 0);

    let mut sess_build = SessionBuilder::<GgrsConfig>::new()
        .with_num_players(4)
        .with_desync_detection_mode(bevy_ggrs::ggrs::DesyncDetection::On { interval: 10 }) // (optional) set how often to exchange state checksums
        .with_max_prediction_window(12) // (optional) set max prediction window
        .with_input_delay(2); // (optional) set input delay for the local player

    for (i, player_addr) in opt.players.iter().enumerate() {
        // local player
        if player_addr == "localhost" {
            sess_build = sess_build.add_player(PlayerType::Local, i).unwrap();
        } else {
            // remote players
            let remote_addr: SocketAddr = player_addr.parse().unwrap();
            sess_build = sess_build
                .add_player(PlayerType::Remote(remote_addr), i)
                .unwrap();
        }
    }

    // optionally, add spectators
    for (i, spec_addr) in opt.spectators.iter().enumerate() {
        sess_build = sess_build
            .add_player(PlayerType::Spectator(*spec_addr), num_players + i)
            .unwrap();
    }

    // start the GGRS session
    let socket = UdpNonBlockingSocket::bind_to_port(opt.local_port).unwrap();
    let sess = sess_build.start_p2p_session(socket).unwrap();

    app.add_ggrs_plugin(
        GgrsPlugin::<GgrsConfig>::new()
            // define frequency of rollback game logic update
            .with_update_frequency(FPS)
            // define system that returns inputs given a player handle, so GGRS can send the inputs around
            .with_input_system(input)
            // register types of components AND resources you want to be rolled back
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
    // these systems will be executed as part of the advance frame update
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
    // add your GGRS session
    .insert_resource(Session::P2P(sess))
    .insert_resource(FrameCount { frame: 0 })
    .add_systems(Update, animate_fuse)
    .run();

    app.run();
}

pub fn increase_frame_system(mut frame_count: ResMut<FrameCount>) {
    frame_count.frame += 1;
}

pub fn input(
    _handle: In<PlayerHandle>,
    mut r: Local<u8>,
    keyboard_input: Res<Input<KeyCode>>,
) -> PlayerInput {
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

    let inp = !*r & input;
    *r = input;

    PlayerInput { inp }
}

pub fn get_battle_mode_map_size_fill(player_count: usize) -> (MapSize, f32) {
    if player_count > 4 {
        (
            MapSize {
                rows: 13,
                columns: 17,
            },
            70.0,
        )
    } else {
        (
            MapSize {
                rows: 11,
                columns: 15,
            },
            60.0,
        )
    }
}

#[derive(Clone, Copy)]
pub enum PenguinControlType {
    Human(usize),
    Bot,
}

pub fn spawn_battle_mode_players(
    commands: &mut Commands,
    game_textures: &GameTextures,
    map_size: MapSize,
    players: &[Penguin],
) -> Vec<Position> {
    let possible_player_spawn_positions = [
        (1, 1),
        (map_size.rows - 2, map_size.columns - 2),
        (1, map_size.columns - 2),
        (map_size.rows - 2, 1),
    ];
    let mut possible_player_spawn_positions =
        possible_player_spawn_positions
            .iter()
            .map(|(y, x)| Position {
                y: *y as isize,
                x: *x as isize,
            });

    let mut player_spawn_positions = vec![];

    let mut spawn_player = |penguin_tag: Penguin| {
        let player_spawn_position = possible_player_spawn_positions.next().unwrap();
        let base_texture = game_textures.get_penguin_texture(penguin_tag).clone();
        let immortal_texture = game_textures.immortal_penguin.clone();
        commands
            .spawn((
                SpriteBundle {
                    texture: base_texture.clone(),
                    transform: Transform::from_xyz(
                        get_x(player_spawn_position.x),
                        get_y(player_spawn_position.y),
                        50.0,
                    ),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(TILE_WIDTH as f32, TILE_HEIGHT as f32)),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                BaseTexture(base_texture),
                ImmortalTexture(immortal_texture),
                Player,
                penguin_tag,
                Health {
                    lives: 1,
                    max_health: 1,
                    health: 1,
                },
                player_spawn_position,
                SpawnPosition(player_spawn_position),
                BombSatchel {
                    bombs_available: 1,
                    bomb_range: 2,
                },
                TeamID(penguin_tag.0),
                HumanControlled(penguin_tag.0),
            ))
            .add_rollback();

        player_spawn_positions.push(player_spawn_position);
    };

    for penguin_tag in players {
        spawn_player(*penguin_tag);
    }

    player_spawn_positions
}

pub fn setup_battle_mode(
    mut commands: Commands,
    mut game_textures: ResMut<GameTextures>,
    fonts: Res<Fonts>,
    hud_colors: Res<HUDColors>,
    mut primary_query: Query<&mut Window, With<PrimaryWindow>>,
) {
    let world_id = WorldID(1);
    game_textures.set_map_textures(world_id);

    let (map_size, percent_of_passable_positions_to_fill) = get_battle_mode_map_size_fill(2);

    // spawn HUD
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..Default::default()
                },
                background_color: Color::NONE.into(),
                ..Default::default()
            },
            UIRoot,
            UIComponent,
        ))
        .with_children(|parent| {
            init_hud(
                parent,
                &hud_colors,
                &fonts,
                (map_size.columns * TILE_WIDTH) as f32,
                world_id,
                true,
                true,
                None,
            );
        });

    let players: Vec<Penguin> = (0..4).map(Penguin).collect();

    // map generation //
    let player_spawn_positions =
        spawn_battle_mode_players(&mut commands, &game_textures, map_size, &players);

    let _ = spawn_map(
        &mut commands,
        &game_textures,
        map_size,
        percent_of_passable_positions_to_fill,
        true,
        &player_spawn_positions,
        &[],
        false,
    );

    primary_query.get_single_mut().unwrap().resolution.set(
        (map_size.columns * TILE_WIDTH) as f32,
        (HUD_HEIGHT + map_size.rows * TILE_HEIGHT) as f32,
    );

    commands.spawn(Camera2dBundle {
        transform: Transform::from_xyz(
            ((map_size.columns * TILE_WIDTH) as f32) / 2.0,
            -((map_size.rows * TILE_HEIGHT - HUD_HEIGHT) as f32 / 2.0),
            999.9,
        ),
        ..default()
    });

    commands.insert_resource(world_id);
}

pub fn player_move(
    inputs: Res<PlayerInputs<GgrsConfig>>,
    mut p: ParamSet<(
        Query<(&mut Transform, &Penguin, &mut Position, &mut Sprite), With<Rollback>>,
        Query<&Position, With<Solid>>,
    )>,
) {
    let solids: HashSet<Position> = p.p1().iter().copied().collect();

    for (mut transform, penguin, mut position, mut sprite) in p.p0().iter_mut() {
        use crate::game::types::Direction;

        let input = inputs[penguin.0].0.inp;
        for (input_mask, direction) in [
            (INPUT_UP, Direction::Up),
            (INPUT_DOWN, Direction::Down),
            (INPUT_LEFT, Direction::Left),
            (INPUT_RIGHT, Direction::Right),
        ] {
            if input & input_mask != 0 {
                // visual / sprite flipping
                match direction {
                    Direction::Left => sprite.flip_x = true,
                    Direction::Right => sprite.flip_x = false,
                    _ => (),
                }

                let new_position = position.offset(direction, 1);
                let solid = solids.get(&new_position);

                let mut moved = false;
                if solid.is_none() {
                    *position = new_position;
                    moved = true;
                }

                if moved {
                    let translation = &mut transform.translation;
                    translation.x = get_x(position.x);
                    translation.y = get_y(position.y);
                }
            }
        }
    }
}

pub fn bomb_drop(
    mut commands: Commands,
    inputs: Res<PlayerInputs<GgrsConfig>>,
    game_textures: Res<GameTextures>,
    fonts: Res<Fonts>,
    world_id: Res<WorldID>,
    mut query: Query<(Entity, &Penguin, &Position, &mut BombSatchel)>,
    query2: Query<&Position, Or<(With<Solid>, With<BurningItem>)>>,
    frame_count: Res<FrameCount>,
) {
    for (entity, penguin, position, mut bomb_satchel) in query.iter_mut() {
        if inputs[penguin.0].0.inp & INPUT_ACTION != 0
            && bomb_satchel.bombs_available > 0
            && !query2.iter().any(|p| *p == *position)
        {
            println!("drop bomb: {:?}", position);
            bomb_satchel.bombs_available -= 1;

            commands
                .spawn((
                    SpriteBundle {
                        texture: game_textures.bomb.clone(),
                        transform: Transform::from_xyz(get_x(position.x), get_y(position.y), 25.0),
                        sprite: Sprite {
                            custom_size: Some(Vec2::new(TILE_WIDTH as f32, TILE_HEIGHT as f32)),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    Bomb {
                        owner: Some(entity),
                        range: bomb_satchel.bomb_range,
                        expiration_frame: frame_count.frame + 2 * FPS,
                    },
                    Solid,
                    *position,
                ))
                .add_rollback()
                .with_children(|parent| {
                    let fuse_color = COLORS[if world_id.0 == 2 { 12 } else { 14 }].into();

                    let mut text = Text::from_section(
                        '*',
                        TextStyle {
                            font: fonts.mono.clone(),
                            font_size: 2.0 * PIXEL_SCALE as f32,
                            color: fuse_color,
                        },
                    )
                    .with_alignment(TextAlignment::Center);
                    text.sections.push(TextSection {
                        value: "┐\n │".into(),
                        style: TextStyle {
                            font: fonts.mono.clone(),
                            font_size: 2.0 * PIXEL_SCALE as f32,
                            color: COLORS[0].into(),
                        },
                    });

                    parent.spawn((
                        Text2dBundle {
                            text,
                            transform: Transform::from_xyz(
                                0.0,
                                TILE_HEIGHT as f32 / 8.0 * 2.0,
                                0.0,
                            ),
                            ..Default::default()
                        },
                        Fuse {
                            color: fuse_color,
                            animation_timer: Timer::from_seconds(0.1, TimerMode::Repeating),
                        },
                    ));
                });
        }
    }
}

pub fn animate_fuse(
    time: Res<Time>,
    fonts: Res<Fonts>,
    query: Query<&Bomb>,
    mut query2: Query<(&Parent, &mut Text, &mut Fuse, &mut Transform)>,
) {
    // for (parent, mut text, mut fuse, mut transform) in query2.iter_mut() {
    //     fuse.animation_timer.tick(time.delta());
    //     let percent_left = fuse.animation_timer.percent_left();
    //     let fuse_char = match percent_left {
    //         _ if (0.0..0.33).contains(&percent_left) => 'x',
    //         _ if (0.33..0.66).contains(&percent_left) => '+',
    //         _ if (0.66..=1.0).contains(&percent_left) => '*',
    //         _ => unreachable!(),
    //     };

    //     let bomb = query.get(parent.get()).unwrap();
    //     let percent_left = bomb.timer.percent_left();

    //     match percent_left {
    //         _ if (0.66..1.0).contains(&percent_left) => {
    //             text.sections = vec![
    //                 TextSection {
    //                     value: fuse_char.into(),
    //                     style: TextStyle {
    //                         font: fonts.mono.clone(),
    //                         font_size: 2.0 * PIXEL_SCALE as f32,
    //                         color: fuse.color,
    //                     },
    //                 },
    //                 TextSection {
    //                     value: "┐\n │".into(),
    //                     style: TextStyle {
    //                         font: fonts.mono.clone(),
    //                         font_size: 2.0 * PIXEL_SCALE as f32,
    //                         color: COLORS[0].into(),
    //                     },
    //                 },
    //             ];
    //             let translation = &mut transform.translation;
    //             translation.x = 0.0;
    //             translation.y = TILE_HEIGHT as f32 / 8.0 * 2.0;
    //         }
    //         _ if (0.33..0.66).contains(&percent_left) => {
    //             text.sections = vec![
    //                 TextSection {
    //                     value: fuse_char.into(),
    //                     style: TextStyle {
    //                         font: fonts.mono.clone(),
    //                         font_size: 2.0 * PIXEL_SCALE as f32,
    //                         color: fuse.color,
    //                     },
    //                 },
    //                 TextSection {
    //                     value: "\n│".into(),
    //                     style: TextStyle {
    //                         font: fonts.mono.clone(),
    //                         font_size: 2.0 * PIXEL_SCALE as f32,
    //                         color: COLORS[0].into(),
    //                     },
    //                 },
    //             ];
    //             let translation = &mut transform.translation;
    //             translation.x = TILE_WIDTH as f32 / 12.0;
    //             translation.y = TILE_HEIGHT as f32 / 8.0 * 2.0;
    //         }
    //         _ if (0.0..0.33).contains(&percent_left) => {
    //             text.sections = vec![TextSection {
    //                 value: fuse_char.into(),
    //                 style: TextStyle {
    //                     font: fonts.mono.clone(),
    //                     font_size: 2.0 * PIXEL_SCALE as f32,
    //                     color: fuse.color,
    //                 },
    //             }];
    //             let translation = &mut transform.translation;
    //             translation.x = TILE_WIDTH as f32 / 12.0;
    //             translation.y = TILE_HEIGHT as f32 / 8.0 * 1.0;
    //         }
    //         _ => (),
    //     }
    // }
}

pub fn fire_tick(
    mut commands: Commands,
    frame_count: Res<FrameCount>,
    query: Query<(Entity, &Fire)>,
) {
    for (entity, fire) in query.iter() {
        if frame_count.frame >= fire.expiration_frame {
            commands.entity(entity).despawn_recursive();
        }
    }
}

pub fn crumbling_tick(
    mut commands: Commands,
    frame_count: Res<FrameCount>,
    query: Query<(Entity, &Crumbling)>,
) {
    for (entity, crumbling) in query.iter() {
        if frame_count.frame >= crumbling.expiration_frame {
            commands.entity(entity).despawn_recursive();
        }
    }
}

pub fn explode_bombs(
    mut commands: Commands,
    game_textures: Res<GameTextures>,
    audio: Res<Audio>,
    // sounds: Res<Sounds>,
    mut p: ParamSet<(
        Query<(Entity, &Bomb, &Position)>,
        Query<(Entity, &Position, Option<&Bomb>), With<Solid>>,
        Query<(&mut Bomb, &Position)>,
    )>,
    mut query3: Query<&mut BombSatchel>,
    mut query: Query<
        (Entity, &Position, &mut Handle<Image>, Option<&Crumbling>),
        (With<Wall>, With<Destructible>),
    >,
    frame_count: Res<FrameCount>,
) {
    let fireproof_positions: HashSet<Position> = p
        .p1()
        .iter()
        .filter_map(|(e, p, b)| {
            // ignore bombs that went off
            if !matches!(b, Some(b) if  frame_count.frame >= b.expiration_frame) {
                Some(p)
            } else {
                None
            }
        })
        .copied()
        .collect();

    let mut sound_played = false;

    let v: Vec<(Entity, Bomb, Position)> = p
        .p0()
        .iter()
        .filter(|(e, b, _)| frame_count.frame >= b.expiration_frame)
        .map(|t| (t.0, t.1.clone(), *t.2))
        .collect();
    for (entity, bomb, position) in v {
        commands.entity(entity).despawn_recursive();

        if let Some(owner) = bomb.owner {
            if let Ok(mut bomb_satchel) = query3.get_mut(owner) {
                bomb_satchel.bombs_available += 1;
            }
        }

        if !sound_played {
            // audio.play(sounds.boom);
            sound_played = true;
        }

        let spawn_fire = |commands: &mut Commands, position: Position| {
            commands
                .spawn((
                    SpriteBundle {
                        texture: game_textures.fire.clone(),
                        transform: Transform::from_xyz(get_x(position.x), get_y(position.y), 5.0),
                        sprite: Sprite {
                            custom_size: Some(Vec2::new(TILE_WIDTH as f32, TILE_HEIGHT as f32)),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    Fire {
                        expiration_frame: frame_count.frame + FPS / 2,
                    },
                    position,
                ))
                .add_rollback();
        };

        spawn_fire(&mut commands, position);
        for direction in crate::game::types::Direction::LIST {
            for i in 1..=bomb.range {
                let position = position.offset(direction, i);

                if fireproof_positions.contains(&position) {
                    // ev_burn.send(BurnEvent { position });

                    // bomb burn
                    p.p2()
                        .iter_mut()
                        .filter(|(_, p)| **p == position)
                        .for_each(|(mut b, _)| {
                            const SHORTENED_FUSE_DURATION: usize = 3;
                            b.expiration_frame = b
                                .expiration_frame
                                .min(frame_count.frame + SHORTENED_FUSE_DURATION);
                        });

                    // destructible wall burn
                    for (e, _, mut t, perishable) in
                        query.iter_mut().filter(|(_, p, _, _)| **p == position)
                    {
                        if perishable.is_none() {
                            commands.entity(e).insert(Crumbling {
                                expiration_frame: frame_count.frame + FPS / 2,
                            });
                            *t = game_textures.get_map_textures().burning_wall.clone();
                        }
                    }

                    break;
                }

                spawn_fire(&mut commands, position);
            }
        }
    }
}
