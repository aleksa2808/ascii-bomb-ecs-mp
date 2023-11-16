use bevy::{prelude::*, utils::HashSet, window::PrimaryWindow};
use bevy_ggrs::{ggrs::SessionBuilder, AddRollbackCommandExtension, PlayerInputs, Session};
use bevy_matchbox::{
    prelude::{PeerState, SingleChannel},
    MatchboxSocket,
};
use rand::seq::IteratorRandom;

use crate::{
    components::*,
    constants::{
        BATTLE_MODE_ROUND_DURATION_SECS, COLORS, FPS, HUD_HEIGHT, INPUT_ACTION, INPUT_DOWN,
        INPUT_LEFT, INPUT_RIGHT, INPUT_UP, PIXEL_SCALE, TILE_HEIGHT, TILE_WIDTH,
    },
    resources::*,
    types::Direction,
    utils::{format_hud_time, get_x, get_y, init_hud, spawn_map},
    AppState, GGRSConfig,
};

pub fn start_matchbox_socket(mut commands: Commands, matchbox_config: Res<MatchboxConfig>) {
    let room_id = match &matchbox_config.room {
        Some(id) => id.clone(),
        None => format!(
            "ascii_bomb_ecs_mp?next={}",
            &matchbox_config.number_of_players
        ),
    };

    let room_url = format!("{}/{}", &matchbox_config.signal_server_address, room_id);
    info!("connecting to matchbox server: {room_url:?}");

    commands.insert_resource(MatchboxSocket::new_ggrs(room_url));
}

pub fn lobby_startup(mut commands: Commands, fonts: Res<Fonts>) {
    commands.spawn(Camera3dBundle::default());

    // All this is just for spawning centered text.
    commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::FlexEnd,
                ..default()
            },
            background_color: Color::rgb(0.43, 0.41, 0.38).into(),
            ..default()
        })
        .with_children(|parent| {
            parent
                .spawn(TextBundle {
                    style: Style {
                        align_self: AlignSelf::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    text: Text::from_section(
                        "Entering lobby...",
                        TextStyle {
                            font: fonts.mono.clone(),
                            font_size: 96.,
                            color: Color::BLACK,
                        },
                    ),
                    ..default()
                })
                .insert(LobbyText);
        })
        .insert(LobbyUI);
}

pub fn lobby_cleanup(
    query: Query<Entity, Or<(With<LobbyUI>, With<Camera3d>)>>,
    mut commands: Commands,
) {
    for e in query.iter() {
        commands.entity(e).despawn_recursive();
    }
}

pub fn lobby_system(
    mut app_state: ResMut<NextState<AppState>>,
    matchbox_config: Res<MatchboxConfig>,
    mut socket: ResMut<MatchboxSocket<SingleChannel>>,
    mut commands: Commands,
    mut query: Query<&mut Text, With<LobbyText>>,
) {
    // regularly call update_peers to update the list of connected peers
    for (peer, new_state) in socket.update_peers() {
        // you can also handle the specific dis(connections) as they occur:
        match new_state {
            PeerState::Connected => info!("peer {peer} connected"),
            PeerState::Disconnected => info!("peer {peer} disconnected"),
        }
    }

    let connected_peers = socket.connected_peers().count();
    let remaining = matchbox_config.number_of_players - (connected_peers + 1);
    query.single_mut().sections[0].value = format!("Waiting for {remaining} more player(s)",);
    if remaining > 0 {
        return;
    }

    info!("All peers have joined, going in-game");

    // extract final player list
    let players = socket.players();
    let player_count = players.len();

    let max_prediction = 12;

    // create a GGRS P2P session
    let mut sess_build = SessionBuilder::<GGRSConfig>::new()
        .with_num_players(matchbox_config.number_of_players)
        .with_desync_detection_mode(bevy_ggrs::ggrs::DesyncDetection::On { interval: 10 })
        .with_max_prediction_window(max_prediction)
        .with_input_delay(2)
        .with_fps(FPS)
        .expect("invalid fps");

    for (i, player) in players.into_iter().enumerate() {
        sess_build = sess_build
            .add_player(player, i)
            .expect("failed to add player");
    }

    let channel = socket.take_channel(0).unwrap();

    // start the GGRS session
    let sess = sess_build
        .start_p2p_session(channel)
        .expect("failed to start session");

    commands.insert_resource(Session::P2P(sess));

    // transition to game state
    commands.insert_resource(Leaderboard {
        scores: (0..player_count).map(|p| (Penguin(p), 0)).collect(),
        winning_score: 3,
    });
    app_state.set(AppState::InGame);
}

pub fn log_ggrs_events(mut session: ResMut<Session<GGRSConfig>>) {
    match session.as_mut() {
        Session::P2P(s) => {
            for event in s.events() {
                info!("GGRS Event: {event:?}");
            }
        }
        _ => panic!("This example focuses on p2p."),
    }
}

pub fn increase_frame_system(mut frame_count: ResMut<FrameCount>) {
    frame_count.frame += 1;
}

pub fn setup_battle_mode(
    mut commands: Commands,
    mut game_textures: ResMut<GameTextures>,
    fonts: Res<Fonts>,
    hud_colors: Res<HUDColors>,
    mut primary_query: Query<&mut Window, With<PrimaryWindow>>,
    matchbox_config: Res<MatchboxConfig>,
    frame_count: Res<FrameCount>,
) {
    let world_id = WorldID(1);
    game_textures.set_map_textures(world_id);

    let (map_size, percent_of_passable_positions_to_fill) = if matchbox_config.number_of_players > 4
    {
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
    };

    let penguin_tags = (0..matchbox_config.number_of_players)
        .map(Penguin)
        .collect::<Vec<Penguin>>();

    commands.insert_resource(GameEndFrame(
        frame_count.frame + BATTLE_MODE_ROUND_DURATION_SECS * FPS,
    ));

    // HUD generation //
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
                &game_textures,
                &penguin_tags,
            );
        });

    // Map generation //
    let possible_player_spawn_positions = [
        (1, 1),
        (map_size.rows - 2, map_size.columns - 2),
        (1, map_size.columns - 2),
        (map_size.rows - 2, 1),
        (3, 5),
        (map_size.rows - 4, map_size.columns - 6),
        (3, map_size.columns - 6),
        (map_size.rows - 4, 5),
    ];
    let mut possible_player_spawn_positions =
        possible_player_spawn_positions
            .iter()
            .map(|(y, x)| Position {
                y: *y as isize,
                x: *x as isize,
            });

    let mut player_spawn_positions = vec![];
    for penguin_tag in penguin_tags {
        let player_spawn_position = possible_player_spawn_positions.next().unwrap();
        let base_texture = game_textures.get_penguin_texture(penguin_tag).clone();
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
                Player,
                penguin_tag,
                player_spawn_position,
                BombSatchel {
                    bombs_available: 1,
                    bomb_range: 2,
                },
            ))
            .add_rollback();

        player_spawn_positions.push(player_spawn_position);
    }

    spawn_map(
        &mut commands,
        &game_textures,
        map_size,
        percent_of_passable_positions_to_fill,
        true,
        &player_spawn_positions,
    );

    // TODO move
    primary_query.get_single_mut().unwrap().resolution.set(
        (map_size.columns * TILE_WIDTH) as f32,
        (HUD_HEIGHT + map_size.rows * TILE_HEIGHT) as f32,
    );

    // TODO move
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

pub fn update_hud_clock(
    game_end_frame: Res<GameEndFrame>,
    mut query: Query<&mut Text, With<GameTimerDisplay>>,
    frame_count: Res<FrameCount>,
    freeze_end_frame: Option<ResMut<FreezeEndFrame>>,
) {
    if freeze_end_frame.is_some() {
        // The current round is over.
        return;
    }

    let remaining_seconds =
        ((game_end_frame.0 - frame_count.frame) as f32 / FPS as f32).ceil() as usize;
    query.single_mut().sections[0].value = format_hud_time(remaining_seconds);
}

pub fn update_player_portraits(
    query: Query<&Penguin>,
    mut query2: Query<(&mut Visibility, &PenguinPortrait)>,
) {
    let players: HashSet<Penguin> = query.iter().copied().collect();

    for (mut visibility, portrait) in query2.iter_mut() {
        if players.contains(&portrait.0) {
            *visibility = Visibility::Visible;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

pub fn player_move(
    inputs: Res<PlayerInputs<GGRSConfig>>,
    mut p: ParamSet<(
        Query<(&mut Transform, &Penguin, &mut Position, &mut Sprite), With<Player>>,
        Query<&Position, With<Solid>>,
    )>,
    freeze_end_frame: Option<ResMut<FreezeEndFrame>>,
) {
    if freeze_end_frame.is_some() {
        // The current round is over.
        // TODO convert into a run criteria
        return;
    }

    let solids: HashSet<Position> = p.p1().iter().copied().collect();

    for (mut transform, penguin, mut position, mut sprite) in p.p0().iter_mut() {
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
    inputs: Res<PlayerInputs<GGRSConfig>>,
    game_textures: Res<GameTextures>,
    fonts: Res<Fonts>,
    world_id: Res<WorldID>,
    mut query: Query<(&Penguin, &Position, &mut BombSatchel), With<Player>>,
    query2: Query<&Position, With<Solid>>,
    frame_count: Res<FrameCount>,
    freeze_end_frame: Option<ResMut<FreezeEndFrame>>,
) {
    if freeze_end_frame.is_some() {
        // The current round is over.
        return;
    }

    for (penguin, position, mut bomb_satchel) in query.iter_mut() {
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
                        owner: Some(*penguin),
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

                    parent
                        .spawn((
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
                                start_frame: frame_count.frame,
                            },
                        ))
                        .add_rollback();
                });
        }
    }
}

pub fn animate_fuse(
    frame_count: Res<FrameCount>,
    fonts: Res<Fonts>,
    query: Query<&Bomb>,
    mut query2: Query<(&Parent, &mut Text, &Fuse, &mut Transform)>,
    freeze_end_frame: Option<ResMut<FreezeEndFrame>>,
) {
    if freeze_end_frame.is_some() {
        // The current round is over.
        return;
    }

    for (parent, mut text, fuse, mut transform) in query2.iter_mut() {
        const FUSE_ANIMATION_FRAME_COUNT: usize = (FPS as f32 * 0.1) as usize;
        // TODO double check calculation
        let percent_left = (FUSE_ANIMATION_FRAME_COUNT
            - (frame_count.frame - fuse.start_frame) % FUSE_ANIMATION_FRAME_COUNT)
            as f32
            / FUSE_ANIMATION_FRAME_COUNT as f32;
        let fuse_char = match percent_left {
            _ if (0.0..0.33).contains(&percent_left) => 'x',
            _ if (0.33..0.66).contains(&percent_left) => '+',
            _ if (0.66..=1.0).contains(&percent_left) => '*',
            _ => unreachable!(),
        };

        let bomb = query.get(parent.get()).unwrap();
        let percent_left = (bomb.expiration_frame - frame_count.frame) as f32
            / (bomb.expiration_frame - fuse.start_frame) as f32;

        match percent_left {
            _ if (0.66..1.0).contains(&percent_left) => {
                text.sections = vec![
                    TextSection {
                        value: fuse_char.into(),
                        style: TextStyle {
                            font: fonts.mono.clone(),
                            font_size: 2.0 * PIXEL_SCALE as f32,
                            color: fuse.color,
                        },
                    },
                    TextSection {
                        value: "┐\n │".into(),
                        style: TextStyle {
                            font: fonts.mono.clone(),
                            font_size: 2.0 * PIXEL_SCALE as f32,
                            color: COLORS[0].into(),
                        },
                    },
                ];
                let translation = &mut transform.translation;
                translation.x = 0.0;
                translation.y = TILE_HEIGHT as f32 / 8.0 * 2.0;
            }
            _ if (0.33..0.66).contains(&percent_left) => {
                text.sections = vec![
                    TextSection {
                        value: fuse_char.into(),
                        style: TextStyle {
                            font: fonts.mono.clone(),
                            font_size: 2.0 * PIXEL_SCALE as f32,
                            color: fuse.color,
                        },
                    },
                    TextSection {
                        value: "\n│".into(),
                        style: TextStyle {
                            font: fonts.mono.clone(),
                            font_size: 2.0 * PIXEL_SCALE as f32,
                            color: COLORS[0].into(),
                        },
                    },
                ];
                let translation = &mut transform.translation;
                translation.x = TILE_WIDTH as f32 / 12.0;
                translation.y = TILE_HEIGHT as f32 / 8.0 * 2.0;
            }
            _ if (0.0..0.33).contains(&percent_left) => {
                text.sections = vec![TextSection {
                    value: fuse_char.into(),
                    style: TextStyle {
                        font: fonts.mono.clone(),
                        font_size: 2.0 * PIXEL_SCALE as f32,
                        color: fuse.color,
                    },
                }];
                let translation = &mut transform.translation;
                translation.x = TILE_WIDTH as f32 / 12.0;
                translation.y = TILE_HEIGHT as f32 / 8.0 * 1.0;
            }
            _ => (),
        }
    }
}

pub fn fire_tick(
    mut commands: Commands,
    frame_count: Res<FrameCount>,
    query: Query<(Entity, &Fire)>,
    freeze_end_frame: Option<ResMut<FreezeEndFrame>>,
) {
    if freeze_end_frame.is_some() {
        // The current round is over.
        return;
    }

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
    freeze_end_frame: Option<ResMut<FreezeEndFrame>>,
) {
    if freeze_end_frame.is_some() {
        // The current round is over.
        return;
    }

    for (entity, crumbling) in query.iter() {
        if frame_count.frame >= crumbling.expiration_frame {
            commands.entity(entity).despawn_recursive();
        }
    }
}

pub fn explode_bombs(
    mut commands: Commands,
    game_textures: Res<GameTextures>,
    mut p: ParamSet<(
        Query<(Entity, &Bomb, &Position)>,
        Query<(Entity, &Position, Option<&Bomb>), With<Solid>>,
        Query<(&mut Bomb, &Position)>,
    )>,
    mut query3: Query<(&Penguin, &mut BombSatchel)>,
    mut query: Query<
        (Entity, &Position, &mut Handle<Image>, Option<&Crumbling>),
        (With<Wall>, With<Destructible>),
    >,
    frame_count: Res<FrameCount>,
    freeze_end_frame: Option<ResMut<FreezeEndFrame>>,
) {
    if freeze_end_frame.is_some() {
        // The current round is over.
        return;
    }

    let fireproof_positions: HashSet<Position> = p
        .p1()
        .iter()
        .filter_map(|(_, p, b)| {
            // ignore bombs that went off
            if !matches!(b, Some(b) if  frame_count.frame >= b.expiration_frame) {
                Some(p)
            } else {
                None
            }
        })
        .copied()
        .collect();

    let v: Vec<(Entity, Bomb, Position)> = p
        .p0()
        .iter()
        .filter(|(_, b, _)| frame_count.frame >= b.expiration_frame)
        .map(|t| (t.0, t.1.clone(), *t.2))
        .collect();
    for (entity, bomb, position) in v {
        commands.entity(entity).despawn_recursive();

        if let Some(owner) = bomb.owner {
            if let Some(mut bomb_satchel) = query3
                .iter_mut()
                .find(|(p, _)| **p == owner)
                .map(|(_, s)| s)
            {
                bomb_satchel.bombs_available += 1;
            }
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
        for direction in Direction::LIST {
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

pub fn player_burn(
    mut commands: Commands,
    query: Query<(Entity, &Position, &Penguin), With<Player>>,
    query2: Query<&Position, With<Fire>>,
    freeze_end_frame: Option<ResMut<FreezeEndFrame>>,
    frame_count: Res<FrameCount>,
) {
    if freeze_end_frame.is_some() {
        // The current round is over.
        return;
    }

    let fire_positions: HashSet<Position> = query2.iter().copied().collect();

    for (e, p, penguin) in query.iter() {
        if fire_positions.contains(p) {
            println!("Player death: {}, position: {p:?}", penguin.0);
            commands.entity(e).remove::<Player>();
            commands.entity(e).insert(Dead {
                cleanup_frame: frame_count.frame + FPS / 2,
            });
        }
    }
}

pub fn finish_round(
    mut commands: Commands,
    query: Query<&Penguin, With<Player>>,
    frame_count: Res<FrameCount>,
    game_end_frame: Res<GameEndFrame>,
    freeze_end_frame: Option<ResMut<FreezeEndFrame>>,
) {
    if freeze_end_frame.is_some() {
        // The current round is over.
        return;
    }

    let mut round_over = false;
    if frame_count.frame >= game_end_frame.0 || query.iter().count() == 0 {
        commands.insert_resource(RoundOutcome::Tie);

        round_over = true;
    } else if let Ok(penguin) = query.get_single() {
        commands.insert_resource(RoundOutcome::Winner(*penguin));

        round_over = true;
    }

    if round_over {
        println!("Round over, freezing...");
        commands.insert_resource(FreezeEndFrame(frame_count.frame + FPS /* 1 second */));
    }
}

pub fn cleanup_dead(
    mut commands: Commands,
    query: Query<(Entity, &Dead)>,
    frame_count: Res<FrameCount>,
    freeze_end_frame: Option<ResMut<FreezeEndFrame>>,
) {
    if freeze_end_frame.is_some() {
        // The current round is over.
        return;
    }

    for (e, d) in query.iter() {
        if frame_count.frame >= d.cleanup_frame {
            commands.entity(e).despawn_recursive();
        }
    }
}

pub fn show_leaderboard(
    mut commands: Commands,
    game_textures: Res<GameTextures>,
    fonts: Res<Fonts>,
    mut leaderboard: ResMut<Leaderboard>,
    round_outcome: Option<Res<RoundOutcome>>,
    freeze_end_frame: Option<ResMut<FreezeEndFrame>>,
    primary_query: Query<&Window, With<PrimaryWindow>>,
    query: Query<Entity, With<UIRoot>>,
    frame_count: Res<FrameCount>,
) {
    if let (Some(mut freeze_end_frame), Some(round_outcome)) =
        (freeze_end_frame, round_outcome.as_deref())
    {
        if frame_count.frame >= freeze_end_frame.0 {
            match round_outcome {
                RoundOutcome::Winner(penguin) => {
                    println!("Winner: {:?}", penguin.0);
                    *leaderboard.scores.get_mut(penguin).unwrap() += 1;
                }
                RoundOutcome::Tie => println!("Tie!"),
            }

            commands.remove_resource::<RoundOutcome>();
            freeze_end_frame.0 = frame_count.frame + 2 * FPS;

            // setup leaderboard display
            let window = primary_query.get_single().unwrap();

            commands.entity(query.single()).with_children(|parent| {
                parent
                    .spawn((
                        NodeBundle {
                            style: Style {
                                position_type: PositionType::Absolute,
                                left: Val::Px(0.0),
                                top: Val::Px(0.0),
                                width: Val::Px(window.width()),
                                height: Val::Px(window.height()),
                                ..Default::default()
                            },
                            background_color: COLORS[0].into(),
                            ..Default::default()
                        },
                        UIComponent,
                        LeaderboardUI,
                    ))
                    .with_children(|parent| {
                        // spawn border
                        let mut spawn_color = |y: usize, x: usize| {
                            parent.spawn((
                                NodeBundle {
                                    style: Style {
                                        position_type: PositionType::Absolute,
                                        left: Val::Px((x * PIXEL_SCALE) as f32),
                                        top: Val::Px((y * PIXEL_SCALE) as f32),
                                        width: Val::Px(PIXEL_SCALE as f32),
                                        height: Val::Px(PIXEL_SCALE as f32),
                                        ..Default::default()
                                    },
                                    background_color: (*COLORS
                                        .iter()
                                        .choose(&mut rand::thread_rng())
                                        .unwrap())
                                    .into(),
                                    ..Default::default()
                                },
                                UIComponent,
                            ));
                        };

                        let height = window.height() as usize / PIXEL_SCALE;
                        let width = window.width() as usize / PIXEL_SCALE;
                        for y in 0..height {
                            spawn_color(y, 0);
                            spawn_color(y, 1);
                            spawn_color(y, width - 2);
                            spawn_color(y, width - 1);
                        }
                        for x in 2..width - 2 {
                            spawn_color(0, x);
                            spawn_color(1, x);
                            spawn_color(height - 2, x);
                            spawn_color(height - 1, x);
                        }

                        for (penguin, score) in &leaderboard.scores {
                            // spawn penguin portrait
                            parent
                                .spawn((
                                    NodeBundle {
                                        style: Style {
                                            position_type: PositionType::Absolute,
                                            left: Val::Px(4.0 * PIXEL_SCALE as f32),
                                            top: Val::Px(
                                                ((6 + penguin.0 * 12) * PIXEL_SCALE) as f32,
                                            ),
                                            width: Val::Px(TILE_WIDTH as f32),
                                            height: Val::Px(TILE_HEIGHT as f32),
                                            ..Default::default()
                                        },
                                        background_color: COLORS[2].into(),
                                        ..Default::default()
                                    },
                                    UIComponent,
                                ))
                                .with_children(|parent| {
                                    parent.spawn((
                                        ImageBundle {
                                            style: Style {
                                                width: Val::Percent(100.0),
                                                height: Val::Percent(100.0),
                                                ..Default::default()
                                            },
                                            image: game_textures
                                                .get_penguin_texture(*penguin)
                                                .clone()
                                                .into(),
                                            ..Default::default()
                                        },
                                        UIComponent,
                                    ));
                                });

                            // spawn penguin trophies
                            for i in 0..*score {
                                parent.spawn((
                                    ImageBundle {
                                        style: Style {
                                            position_type: PositionType::Absolute,
                                            top: Val::Px(
                                                ((7 + penguin.0 * 12) * PIXEL_SCALE) as f32,
                                            ),
                                            left: Val::Px(((15 + i * 9) * PIXEL_SCALE) as f32),
                                            width: Val::Px(5.0 * PIXEL_SCALE as f32),
                                            height: Val::Px(7.0 * PIXEL_SCALE as f32),
                                            ..Default::default()
                                        },
                                        image: game_textures.trophy.clone().into(),
                                        ..Default::default()
                                    },
                                    UIComponent,
                                ));
                            }

                            if let RoundOutcome::Winner(round_winner_penguin) = round_outcome {
                                if penguin == round_winner_penguin {
                                    let mut place_text = |y, x, str: &str, c: usize| {
                                        parent.spawn((
                                            TextBundle {
                                                text: Text::from_section(
                                                    str.to_string(),
                                                    TextStyle {
                                                        font: fonts.mono.clone(),
                                                        font_size: 2.0 * PIXEL_SCALE as f32,
                                                        color: COLORS[c].into(),
                                                    },
                                                ),
                                                style: Style {
                                                    position_type: PositionType::Absolute,
                                                    top: Val::Px(y as f32 * PIXEL_SCALE as f32),
                                                    left: Val::Px(x as f32 * PIXEL_SCALE as f32),
                                                    ..Default::default()
                                                },
                                                ..Default::default()
                                            },
                                            UIComponent,
                                        ));
                                    };

                                    place_text(
                                        6 + penguin.0 * 12,
                                        15 + (score - 1) * 9 - 2,
                                        "*",
                                        15,
                                    );
                                    place_text(
                                        8 + penguin.0 * 12,
                                        15 + (score - 1) * 9 + 6,
                                        "*",
                                        15,
                                    );
                                    place_text(
                                        10 + penguin.0 * 12,
                                        15 + (score - 1) * 9 - 1,
                                        "*",
                                        15,
                                    );
                                }
                            }
                        }
                    });
            });
        }
    }
}

pub fn start_new_round(
    mut commands: Commands,
    freeze_end_frame: Option<ResMut<FreezeEndFrame>>,
    round_outcome: Option<Res<RoundOutcome>>,
    tournament_complete: Option<Res<TournamentComplete>>,
    frame_count: Res<FrameCount>,
    leaderboard: Res<Leaderboard>,
    query: Query<Entity, Without<Window>>,
    matchbox_config: Res<MatchboxConfig>,
    game_textures: ResMut<GameTextures>,
    fonts: Res<Fonts>,
    hud_colors: Res<HUDColors>,
    primary_query: Query<&mut Window, With<PrimaryWindow>>,
) {
    if let (Some(mut freeze_end_frame), None, None) =
        (freeze_end_frame, round_outcome, tournament_complete)
    {
        if frame_count.frame >= freeze_end_frame.0 {
            if let Some((p, _)) = leaderboard
                .scores
                .iter()
                .find(|(_, u)| **u >= leaderboard.winning_score)
            {
                println!("Penguin {} WINNER WINNER CHICKEN DINNER", p.0);
                freeze_end_frame.0 = frame_count.frame + 5 * FPS;
                commands.insert_resource(TournamentComplete);

                // TODO show winner
            } else {
                commands.remove_resource::<FreezeEndFrame>();

                for e in query.iter() {
                    // TODO should everything be rollbackable now?
                    commands.entity(e).despawn();
                }

                setup_battle_mode(
                    commands,
                    game_textures,
                    fonts,
                    hud_colors,
                    primary_query,
                    matchbox_config,
                    frame_count,
                )
            }
        }
    }
}

pub fn start_new_tournament(
    mut commands: Commands,
    query: Query<Entity, Without<Window>>,
    freeze_end_frame: Option<Res<FreezeEndFrame>>,
    tournament_complete: Option<Res<TournamentComplete>>,
    frame_count: Res<FrameCount>,
    mut leaderboard: ResMut<Leaderboard>,
    game_textures: ResMut<GameTextures>,
    fonts: Res<Fonts>,
    hud_colors: Res<HUDColors>,
    primary_query: Query<&mut Window, With<PrimaryWindow>>,
    matchbox_config: Res<MatchboxConfig>,
) {
    if let (Some(freeze_end_frame), Some(_)) = (freeze_end_frame, tournament_complete) {
        if frame_count.frame >= freeze_end_frame.0 {
            commands.remove_resource::<FreezeEndFrame>();
            commands.remove_resource::<TournamentComplete>();

            for (_, score) in &mut leaderboard.scores {
                *score = 0;
            }

            for e in query.iter() {
                // TODO should everything be rollbackable now?
                commands.entity(e).despawn();
            }

            setup_battle_mode(
                commands,
                game_textures,
                fonts,
                hud_colors,
                primary_query,
                matchbox_config,
                frame_count,
            )
        }
    }
}
