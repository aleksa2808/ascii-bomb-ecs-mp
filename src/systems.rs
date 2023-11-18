use bevy::{
    prelude::*,
    utils::{HashMap, HashSet},
    window::PrimaryWindow,
};
use bevy_ggrs::{ggrs::SessionBuilder, AddRollbackCommandExtension, PlayerInputs, Session};
use bevy_matchbox::{
    matchbox_socket::{MultipleChannels, WebRtcSocketBuilder},
    prelude::PeerState,
    MatchboxSocket,
};
use rand::{rngs::StdRng, seq::IteratorRandom, Rng, SeedableRng};

use crate::{
    components::*,
    constants::{
        COLORS, FPS, HUD_HEIGHT, INPUT_ACTION, INPUT_DOWN, INPUT_LEFT, INPUT_RIGHT, INPUT_UP,
        ITEM_SPAWN_CHANCE_PERCENTAGE, PIXEL_SCALE, TILE_HEIGHT, TILE_WIDTH,
    },
    resources::*,
    types::Direction,
    utils::{format_hud_time, generate_item_at_position, get_x, get_y, setup_round},
    AppState, GgrsConfig,
};

pub fn setup_lobby(mut commands: Commands, fonts: Res<Fonts>) {
    commands.spawn(Camera2dBundle::default());

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

    let socket = WebRtcSocketBuilder::new(room_url)
        .add_ggrs_channel()
        .add_reliable_channel()
        .build();
    commands.insert_resource(MatchboxSocket::from(socket));

    let local_seed = rand::random();
    info!("Generated the local RNG seed: {local_seed}");
    commands.insert_resource(RngSeeds {
        local: local_seed,
        remote: HashMap::with_capacity(matchbox_config.number_of_players - 1),
    });
}

pub fn lobby_system(
    mut app_state: ResMut<NextState<AppState>>,
    matchbox_config: Res<MatchboxConfig>,
    mut socket: ResMut<MatchboxSocket<MultipleChannels>>,
    mut rng_seeds: ResMut<RngSeeds>,
    mut commands: Commands,
    mut query: Query<&mut Text, With<LobbyText>>,
) {
    // regularly call update_peers to update the list of connected peers
    for (peer, new_state) in socket.update_peers() {
        // you can also handle the specific dis(connections) as they occur:
        match new_state {
            PeerState::Connected => {
                info!("peer {peer} connected, sending them our local RNG seed");
                let packet = rng_seeds.local.to_be_bytes().to_vec().into_boxed_slice();
                socket.channel(1).send(packet, peer);
            }
            PeerState::Disconnected => info!("peer {peer} disconnected"),
        }
    }

    for (peer, packet) in socket.channel(1).receive() {
        // decode the message
        assert!(packet.len() == 8);
        let mut remote_seed = [0; 8];
        packet
            .iter()
            .enumerate()
            .for_each(|(i, &b)| remote_seed[i] = b);
        let remote_seed = u64::from_be_bytes(remote_seed);

        info!("Got RNG seed from {peer}: {remote_seed}");
        let old_value = rng_seeds.remote.insert(peer, remote_seed);
        assert!(
            old_value.is_none(),
            "Received RNG seed from {} twice!",
            peer
        );
    }

    let remaining = matchbox_config.number_of_players - (rng_seeds.remote.len() + 1);
    query.single_mut().sections[0].value = format!("Waiting for {remaining} more player(s)");
    if remaining > 0 {
        return;
    }

    let shared_seed = rng_seeds.local
        ^ rng_seeds
            .remote
            .values()
            .copied()
            .reduce(|acc, e| acc ^ e)
            .unwrap();
    info!("Generated the shared RNG seed: {shared_seed}");
    commands.remove_resource::<RngSeeds>();
    commands.insert_resource(SessionRng(StdRng::seed_from_u64(shared_seed)));

    info!("All peers have joined and the shared RNG seed was created, going in-game");

    // extract final player list
    let players = socket.players();
    let player_count = players.len();

    let max_prediction = 12;

    let mut sess_build = SessionBuilder::<GgrsConfig>::new()
        .with_num_players(matchbox_config.number_of_players)
        .with_desync_detection_mode(bevy_ggrs::ggrs::DesyncDetection::On { interval: 10 })
        .with_max_prediction_window(max_prediction)
        .expect("prediction window can't be 0")
        .with_input_delay(2);

    for (i, player) in players.into_iter().enumerate() {
        sess_build = sess_build
            .add_player(player, i)
            .expect("failed to add player");
    }

    let channel = socket.take_channel(0).unwrap();

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

pub fn teardown_lobby(
    query: Query<Entity, Or<(With<LobbyUI>, With<Camera2d>)>>,
    mut commands: Commands,
) {
    for e in query.iter() {
        commands.entity(e).despawn_recursive();
    }
}

pub fn log_ggrs_events(mut session: ResMut<Session<GgrsConfig>>) {
    match session.as_mut() {
        Session::P2P(s) => {
            for event in s.events() {
                info!("GgrsEvent: {event:?}");
            }
        }
        _ => unreachable!(),
    }
}

pub fn setup_game(
    mut commands: Commands,
    mut session_rng: ResMut<SessionRng>,
    game_textures: Res<GameTextures>,
    fonts: Res<Fonts>,
    hud_colors: Res<HUDColors>,
    mut primary_query: Query<&mut Window, With<PrimaryWindow>>,
    matchbox_config: Res<MatchboxConfig>,
    frame_count: Res<FrameCount>,
) {
    let map_size = if matchbox_config.number_of_players > 4 {
        MapSize {
            rows: 13,
            columns: 17,
        }
    } else {
        MapSize {
            rows: 11,
            columns: 15,
        }
    };
    commands.insert_resource(map_size);

    // resize window based on map size
    primary_query.get_single_mut().unwrap().resolution.set(
        (map_size.columns * TILE_WIDTH) as f32,
        (HUD_HEIGHT + map_size.rows * TILE_HEIGHT) as f32,
    );

    // spawn the main game camera
    commands.spawn(Camera2dBundle {
        transform: Transform::from_xyz(
            ((map_size.columns * TILE_WIDTH) as f32) / 2.0,
            -((map_size.rows * TILE_HEIGHT - HUD_HEIGHT) as f32 / 2.0),
            999.9,
        ),
        ..default()
    });

    // choose the initial world
    let world_type = WorldType::random(&mut session_rng.0);
    commands.insert_resource(world_type);

    setup_round(
        &mut session_rng.0,
        commands,
        map_size,
        world_type,
        &game_textures,
        &fonts,
        &hud_colors,
        matchbox_config.number_of_players,
        frame_count.frame,
    );
}

pub fn increase_frame_system(mut frame_count: ResMut<FrameCount>) {
    frame_count.frame += 1;
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
    inputs: Res<PlayerInputs<GgrsConfig>>,
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

pub fn pick_up_item(
    mut commands: Commands,
    game_textures: Res<GameTextures>,
    mut query: Query<(Entity, &Position, &mut BombSatchel), With<Player>>,
    query2: Query<(Entity, &Item, &Position)>,
    frame_count: Res<FrameCount>,
    freeze_end_frame: Option<ResMut<FreezeEndFrame>>,
) {
    if freeze_end_frame.is_some() {
        // The current round is over.
        return;
    }

    for (ie, i, ip) in query2.iter() {
        let mut it = query
            .iter_mut()
            .filter_map(|(e, pp, bs)| if *pp == *ip { Some((e, bs)) } else { None });
        match (it.next(), it.next()) {
            (None, None) => {
                // There are no players at this position
            }
            (Some((_pe, mut bomb_satchel)), None) => {
                println!("powered up: {:?}", ip);
                match i {
                    Item::BombsUp => bomb_satchel.bombs_available += 1,
                    Item::RangeUp => bomb_satchel.bomb_range += 1,
                    Item::BombPush => {
                        // commands.entity(pe).insert(BombPush);
                    }
                };

                commands.entity(ie).despawn_recursive();
            }
            (Some(_), Some(_)) => {
                println!("Multiple players arrived at item position ({:?}) at the same time! In the ensuing chaos the item was destroyed...", ip);
                commands.entity(ie).despawn_recursive();
                commands.spawn((
                    SpriteBundle {
                        texture: game_textures.burning_item.clone(),
                        transform: Transform::from_xyz(get_x(ip.x), get_y(ip.y), 20.0),
                        sprite: Sprite {
                            custom_size: Some(Vec2::new(TILE_WIDTH as f32, TILE_HEIGHT as f32)),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    *ip,
                    BurningItem {
                        expiration_frame: frame_count.frame + FPS / 2,
                    },
                ));
            }
            (None, Some(_)) => unreachable!(),
        }
    }
}

pub fn bomb_drop(
    mut commands: Commands,
    inputs: Res<PlayerInputs<GgrsConfig>>,
    game_textures: Res<GameTextures>,
    fonts: Res<Fonts>,
    world_type: Res<WorldType>,
    mut query: Query<(&Penguin, &Position, &mut BombSatchel), With<Player>>,
    query2: Query<&Position, Or<(With<Solid>, With<BurningItem>)>>,
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
                    let fuse_color = COLORS[match *world_type {
                        WorldType::GrassWorld | WorldType::CloudWorld => 14,
                        WorldType::IceWorld => 12,
                    }]
                    .into();

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
    mut session_rng: ResMut<SessionRng>,
    frame_count: Res<FrameCount>,
    query: Query<(Entity, &Crumbling, &Position)>,
    game_textures: Res<GameTextures>,
    freeze_end_frame: Option<ResMut<FreezeEndFrame>>,
) {
    if freeze_end_frame.is_some() {
        // The current round is over.
        return;
    }

    for (entity, crumbling, position) in query.iter() {
        if frame_count.frame >= crumbling.expiration_frame {
            commands.entity(entity).despawn_recursive();

            // drop power-up
            let r = session_rng.0.gen_range(0..100);
            if r < ITEM_SPAWN_CHANCE_PERCENTAGE {
                generate_item_at_position(
                    &mut session_rng.0,
                    *position,
                    &mut commands,
                    &game_textures,
                );
            }
        }
    }
}

pub fn burning_item_tick(
    mut commands: Commands,
    frame_count: Res<FrameCount>,
    query: Query<(Entity, &BurningItem)>,
    freeze_end_frame: Option<ResMut<FreezeEndFrame>>,
) {
    if freeze_end_frame.is_some() {
        // The current round is over.
        return;
    }

    for (entity, burning_item) in query.iter() {
        if frame_count.frame >= burning_item.expiration_frame {
            commands.entity(entity).despawn_recursive();
        }
    }
}

pub fn explode_bombs(
    mut commands: Commands,
    world_type: Res<WorldType>,
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
        .map(|t| (t.0, *t.1, *t.2))
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
                            *t = game_textures
                                .get_map_textures(*world_type)
                                .burning_wall
                                .clone();
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
    frame_count: Res<FrameCount>,
    freeze_end_frame: Option<ResMut<FreezeEndFrame>>,
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

pub fn item_burn(
    mut commands: Commands,
    game_textures: Res<GameTextures>,
    query: Query<(Entity, &Position), With<Item>>,
    query2: Query<&Position, With<Fire>>,
    frame_count: Res<FrameCount>,
    freeze_end_frame: Option<ResMut<FreezeEndFrame>>,
) {
    if freeze_end_frame.is_some() {
        // The current round is over.
        return;
    }

    let fire_positions: HashSet<Position> = query2.iter().copied().collect();

    for (entity, position) in query.iter() {
        if fire_positions.contains(position) {
            commands.entity(entity).despawn_recursive();
            commands.spawn((
                SpriteBundle {
                    texture: game_textures.burning_item.clone(),
                    transform: Transform::from_xyz(get_x(position.x), get_y(position.y), 20.0),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(TILE_WIDTH as f32, TILE_HEIGHT as f32)),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                *position,
                BurningItem {
                    expiration_frame: frame_count.frame + FPS / 2,
                },
            ));
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
    mut session_rng: ResMut<SessionRng>,
    freeze_end_frame: Option<ResMut<FreezeEndFrame>>,
    round_outcome: Option<Res<RoundOutcome>>,
    tournament_complete: Option<Res<TournamentComplete>>,
    frame_count: Res<FrameCount>,
    leaderboard: Res<Leaderboard>,
    query: Query<Entity, (Without<Window>, Without<Camera2d>)>,
    map_size: Res<MapSize>,
    world_type: Res<WorldType>,
    matchbox_config: Res<MatchboxConfig>,
    game_textures: ResMut<GameTextures>,
    fonts: Res<Fonts>,
    hud_colors: Res<HUDColors>,
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

                setup_round(
                    &mut session_rng.0,
                    commands,
                    *map_size,
                    *world_type,
                    &game_textures,
                    &fonts,
                    &hud_colors,
                    matchbox_config.number_of_players,
                    frame_count.frame,
                )
            }
        }
    }
}

pub fn start_new_tournament(
    mut commands: Commands,
    mut session_rng: ResMut<SessionRng>,
    freeze_end_frame: Option<Res<FreezeEndFrame>>,
    tournament_complete: Option<Res<TournamentComplete>>,
    frame_count: Res<FrameCount>,
    mut leaderboard: ResMut<Leaderboard>,
    query: Query<Entity, (Without<Window>, Without<Camera2d>)>,
    map_size: Res<MapSize>,
    mut world_type: ResMut<WorldType>,
    matchbox_config: Res<MatchboxConfig>,
    game_textures: ResMut<GameTextures>,
    fonts: Res<Fonts>,
    hud_colors: Res<HUDColors>,
) {
    if let (Some(freeze_end_frame), Some(_)) = (freeze_end_frame, tournament_complete) {
        if frame_count.frame >= freeze_end_frame.0 {
            // clear game state
            for e in query.iter() {
                // TODO should everything be rollbackable now?
                commands.entity(e).despawn();
            }
            commands.remove_resource::<FreezeEndFrame>();
            commands.remove_resource::<TournamentComplete>();

            // reset the leaderboard
            for (_, score) in &mut leaderboard.scores {
                *score = 0;
            }

            // change the tournament world
            *world_type = world_type.next_random(&mut session_rng.0);

            setup_round(
                &mut session_rng.0,
                commands,
                *map_size,
                *world_type,
                &game_textures,
                &fonts,
                &hud_colors,
                matchbox_config.number_of_players,
                frame_count.frame,
            )
        }
    }
}
