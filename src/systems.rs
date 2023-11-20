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
use itertools::Itertools;
use rand::{rngs::StdRng, Rng, SeedableRng};

use crate::{
    components::*,
    constants::{
        BOMB_Z_LAYER, COLORS, FIRE_Z_LAYER, FPS, GAME_START_FREEZE_FRAME_COUNT, HUD_HEIGHT,
        INPUT_ACTION, INPUT_DOWN, INPUT_LEFT, INPUT_RIGHT, INPUT_UP, ITEM_SPAWN_CHANCE_PERCENTAGE,
        LEADERBOARD_DISPLAY_FRAME_COUNT, PIXEL_SCALE, PLAYER_DEATH_FRAME_DELAY, TILE_HEIGHT,
        TILE_WIDTH, TOURNAMENT_WINNER_DISPLAY_FRAME_COUNT, WALL_Z_LAYER,
    },
    resources::*,
    types::{Direction, PlayerID, PostFreezeAction, RoundOutcome},
    utils::{
        format_hud_time, generate_item_at_position, get_x, get_y, setup_leaderboard_display,
        setup_round, spawn_burning_item,
    },
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

    commands.insert_resource(MatchboxSocket::from(
        WebRtcSocketBuilder::new(room_url)
            .add_ggrs_channel()
            .add_reliable_channel()
            .build(),
    ));

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
        .with_desync_detection_mode(bevy_ggrs::ggrs::DesyncDetection::On { interval: 1 })
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
        scores: (0..player_count).map(|p| (PlayerID(p), 0)).collect(),
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
                // TODO do something on desyncs
            }
        }
        _ => unreachable!(),
    }
}

pub fn setup_game(
    mut commands: Commands,
    mut session_rng: ResMut<SessionRng>,
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

    // TODO this is kind of a hack
    commands.insert_resource(GameFreeze {
        end_frame: frame_count.frame, /* this frame */
        post_freeze_action: Some(PostFreezeAction::StartNewRound),
    })
}

pub fn increase_frame_system(mut frame_count: ResMut<FrameCount>) {
    frame_count.frame += 1;
}

pub fn update_hud_clock(
    game_end_frame: Res<GameEndFrame>,
    mut query: Query<&mut Text, With<GameTimerDisplay>>,
    frame_count: Res<FrameCount>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    let remaining_seconds =
        ((game_end_frame.0 - frame_count.frame) as f32 / FPS as f32).ceil() as usize;
    query.single_mut().sections[0].value = format_hud_time(remaining_seconds);
}

pub fn update_player_portraits(
    query: Query<&Player>,
    mut query2: Query<(&mut Visibility, &PlayerPortrait)>,
) {
    let player_ids: HashSet<PlayerID> = query.iter().map(|player| player.id).collect();

    for (mut visibility, portrait) in query2.iter_mut() {
        if player_ids.contains(&portrait.0) {
            *visibility = Visibility::Visible;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

pub fn player_move(
    inputs: Res<PlayerInputs<GgrsConfig>>,
    mut p: ParamSet<(
        Query<(&Player, &mut Position, &mut Transform, &mut Sprite), Without<Dead>>,
        Query<(Entity, &Position, Option<&Bomb>), With<Solid>>,
        Query<&mut Bomb>,
    )>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    let solids: HashMap<Position, Option<Entity>> = p
        .p1()
        .iter()
        .map(|(solid_entity, solid_position, optional_bomb)| {
            (*solid_position, optional_bomb.map(|_| solid_entity))
        })
        .collect();

    let mut bomb_move_vec = vec![];
    for (player, mut position, mut transform, mut sprite) in p.p0().iter_mut() {
        let input = inputs[player.id.0].0 .0;
        for (input_mask, moving_direction) in [
            (INPUT_UP, Direction::Up),
            (INPUT_DOWN, Direction::Down),
            (INPUT_LEFT, Direction::Left),
            (INPUT_RIGHT, Direction::Right),
        ] {
            if input & input_mask != 0 {
                // visual / sprite flipping
                match moving_direction {
                    Direction::Left => sprite.flip_x = true,
                    Direction::Right => sprite.flip_x = false,
                    _ => (),
                }

                let new_position = position.offset(moving_direction, 1);
                let solid = solids.get(&new_position);

                if let Some(&optional_bomb_entity) = solid {
                    if player.can_push_bombs {
                        if let Some(bomb_entity) = optional_bomb_entity {
                            // TODO figure out how to get a &mut Bomb from the solids query in order to avoid this workaround
                            bomb_move_vec.push((bomb_entity, moving_direction));
                        }
                    }
                } else {
                    *position = new_position;
                    let translation = &mut transform.translation;
                    translation.x = get_x(position.x);
                    translation.y = get_y(position.y);
                }
            }
        }
    }

    for (bomb_entity, direction) in bomb_move_vec {
        p.p2().get_mut(bomb_entity).unwrap().moving = Some(direction);
    }
}

pub fn bomb_move(
    mut p: ParamSet<(
        Query<(&mut Bomb, &mut Position, &mut Transform)>,
        Query<&Position, Or<(With<Solid>, With<Item>, With<Player>)>>,
    )>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    let impassable_positions: HashSet<Position> = p.p1().iter().copied().collect();

    for (mut bomb, mut position, mut transform) in p.p0().iter_mut() {
        if let Some(direction) = bomb.moving {
            // TODO bomb movement is fixed to once per frame
            let new_position = position.offset(direction, 1);
            if impassable_positions.get(&new_position).is_none() {
                *position = new_position;

                let translation = &mut transform.translation;
                translation.x = get_x(position.x);
                translation.y = get_y(position.y);
            } else {
                bomb.moving = None;
            }
        }
    }
}

pub fn pick_up_item(
    mut commands: Commands,
    game_textures: Res<GameTextures>,
    mut query: Query<(&mut Player, &Position, &mut BombSatchel), Without<Dead>>,
    query2: Query<(Entity, &Item, &Position)>,
    frame_count: Res<FrameCount>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    for (item_entity, &item, &item_position) in query2.iter() {
        let mut players_at_item_position =
            query
                .iter_mut()
                .filter_map(|(player, &player_position, bomb_satchel)| {
                    (player_position == item_position).then_some((player, bomb_satchel))
                });
        match (
            players_at_item_position.next(),
            players_at_item_position.next(),
        ) {
            (None, None) => {
                // There are no players at this position
            }
            (Some((mut player, mut bomb_satchel)), None) => {
                println!("powered up: {item_position:?}");
                match item {
                    Item::BombsUp => bomb_satchel.bombs_available += 1,
                    Item::RangeUp => bomb_satchel.bomb_range += 1,
                    Item::BombPush => {
                        player.can_push_bombs = true;
                    }
                };

                commands.entity(item_entity).despawn_recursive();
            }
            (Some(_), Some(_)) => {
                println!("Multiple players arrived at item position ({item_position:?}) at the same time! In the ensuing chaos the item was destroyed...");
                commands.entity(item_entity).despawn_recursive();
                spawn_burning_item(
                    &mut commands,
                    &game_textures,
                    item_position,
                    frame_count.frame,
                );
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
    mut query: Query<(&Player, &Position, &mut BombSatchel), Without<Dead>>,
    query2: Query<&Position, Or<(With<Solid>, With<BurningItem>)>>,
    frame_count: Res<FrameCount>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    for (player, position, mut bomb_satchel) in query.iter_mut() {
        if inputs[player.id.0].0 .0 & INPUT_ACTION != 0
            && bomb_satchel.bombs_available > 0
            && !query2.iter().any(|p| *p == *position)
        {
            println!("drop bomb: {:?}", position);
            bomb_satchel.bombs_available -= 1;

            commands
                .spawn((
                    SpriteBundle {
                        texture: game_textures.bomb.clone(),
                        transform: Transform::from_xyz(
                            get_x(position.x),
                            get_y(position.y),
                            BOMB_Z_LAYER,
                        ),
                        sprite: Sprite {
                            custom_size: Some(Vec2::new(TILE_WIDTH as f32, TILE_HEIGHT as f32)),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    Bomb {
                        owner: Some(player.id),
                        range: bomb_satchel.bomb_range,
                        expiration_frame: frame_count.frame + 2 * FPS,
                        moving: None,
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
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
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
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
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
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    for (entity, _, position) in query
        .iter()
        .filter(|(_, c, _)| frame_count.frame >= c.expiration_frame)
        .sorted_unstable_by_key(|(_, _, &p)| p)
    {
        commands.entity(entity).despawn_recursive();

        // drop power-up
        let roll = session_rng.0.gen_range(0..100);
        if roll < ITEM_SPAWN_CHANCE_PERCENTAGE {
            generate_item_at_position(&mut session_rng.0, &mut commands, &game_textures, *position);
        }
    }
}

pub fn burning_item_tick(
    mut commands: Commands,
    frame_count: Res<FrameCount>,
    query: Query<(Entity, &BurningItem)>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    for (entity, _) in query
        .iter()
        .filter(|(_, bi)| frame_count.frame >= bi.expiration_frame)
    {
        commands.entity(entity).despawn_recursive();
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
    mut query3: Query<(&Player, &mut BombSatchel), Without<Dead>>,
    mut query: Query<
        (Entity, &Position, &mut Handle<Image>, Option<&Crumbling>),
        (With<Wall>, With<Destructible>),
    >,
    query2: Query<(Entity, &Position), With<Fire>>,
    frame_count: Res<FrameCount>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
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
            if let Some((_, mut bomb_satchel)) =
                query3.iter_mut().find(|(player, _)| player.id == owner)
            {
                bomb_satchel.bombs_available += 1;
            }
        }

        let spawn_fire = |commands: &mut Commands, position: Position| {
            // remove previous fire at position if it exists
            for (e, _) in query2.iter().filter(|(_, &p)| p == position) {
                commands.entity(e).despawn_recursive();
            }

            commands
                .spawn((
                    SpriteBundle {
                        texture: game_textures.fire.clone(),
                        transform: Transform::from_xyz(
                            get_x(position.x),
                            get_y(position.y),
                            FIRE_Z_LAYER,
                        ),
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
    query: Query<(Entity, &Player, &Position), Without<Dead>>,
    query2: Query<&Position, With<Fire>>,
    frame_count: Res<FrameCount>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    let fire_positions: HashSet<Position> = query2.iter().copied().collect();

    for (entity, player, position) in query.iter() {
        if fire_positions.contains(position) {
            println!("Player burned: {}, position: {position:?}", player.id.0);
            commands.entity(entity).insert(Dead {
                cleanup_frame: frame_count.frame + PLAYER_DEATH_FRAME_DELAY,
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
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    let fire_positions: HashSet<Position> = query2.iter().copied().collect();

    for (entity, &position) in query.iter().filter(|(_, p)| fire_positions.contains(*p)) {
        commands.entity(entity).despawn_recursive();
        spawn_burning_item(&mut commands, &game_textures, position, frame_count.frame);
    }
}

pub fn wall_of_death_update(
    mut commands: Commands,
    game_textures: Res<GameTextures>,
    mut wall_of_death: ResMut<WallOfDeath>,
    world_type: Res<WorldType>,
    map_size: Res<MapSize>,
    query: Query<&Position, (With<Wall>, Without<Destructible>)>,
    query2: Query<(Entity, &Position, Option<&Bomb>)>,
    mut query3: Query<(&Player, &mut BombSatchel, Option<&Dead>)>,
    frame_count: Res<FrameCount>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    let get_next_position_direction = |mut position: Position,
                                       mut direction: Direction|
     -> Option<(Position, Direction)> {
        let end_position = Position {
            y: map_size.rows as isize - 3,
            x: 3,
        };

        let walls: HashSet<Position> = query.iter().copied().collect();
        loop {
            if position == end_position {
                break None;
            }

            match position {
                Position { y: 1, x: 1 } | Position { y: 2, x: 2 } => {
                    direction = Direction::Right;
                }
                Position { y: 1, x } if x == map_size.columns as isize - 2 => {
                    direction = Direction::Down;
                }
                Position { y, x }
                    if y == map_size.rows as isize - 2 && x == map_size.columns as isize - 2 =>
                {
                    direction = Direction::Left;
                }
                Position { y, x: 2 } if y == map_size.rows as isize - 2 => {
                    direction = Direction::Up;
                }
                Position { y: 2, x } if x == map_size.columns as isize - 3 => {
                    direction = Direction::Down;
                }
                Position { y, x }
                    if y == map_size.rows as isize - 3 && x == map_size.columns as isize - 3 =>
                {
                    direction = Direction::Left;
                }
                _ => (),
            }

            position = position.offset(direction, 1);
            if !walls.contains(&position) {
                break Some((position, direction));
            }
        }
    };

    let mut clear_position_and_spawn_wall = |position: Position| {
        for (entity, position, bomb) in query2.iter().filter(|(_, &p, _)| p == position) {
            if let Ok((player, _, dead)) = query3.get(entity) {
                if dead.is_none() {
                    println!("Player crushed: {}, position: {position:?}", player.id.0);
                    commands.entity(entity).insert(Dead {
                        cleanup_frame: frame_count.frame + PLAYER_DEATH_FRAME_DELAY,
                    });
                }
            } else {
                commands.entity(entity).despawn_recursive();
            }

            if let Some(bomb) = bomb {
                if let Some(owner) = bomb.owner {
                    if let Some((_, mut bomb_satchel, _)) = query3
                        .iter_mut()
                        .filter(|(_, _, dead)| dead.is_none())
                        .find(|(&player, _, _)| player.id == owner)
                    {
                        bomb_satchel.bombs_available += 1;
                    }
                }
            }
        }

        commands
            .spawn((
                SpriteBundle {
                    texture: game_textures.get_map_textures(*world_type).wall.clone(),
                    transform: Transform::from_xyz(
                        get_x(position.x),
                        get_y(position.y),
                        WALL_Z_LAYER,
                    ),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(TILE_WIDTH as f32, TILE_HEIGHT as f32)),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                Wall,
                Solid,
                position,
            ))
            .add_rollback();
    };

    loop {
        let new_state = match *wall_of_death {
            WallOfDeath::Dormant { activation_frame } => {
                if frame_count.frame >= activation_frame {
                    println!("Wall of Death activated!");

                    Some(WallOfDeath::Active {
                        position: Position {
                            y: map_size.rows as isize - 1,
                            x: 1,
                        },
                        direction: Direction::Up,
                        next_step_frame: frame_count.frame,
                    })
                } else {
                    None
                }
            }
            WallOfDeath::Active {
                ref mut position,
                ref mut direction,
                ref mut next_step_frame,
            } => {
                if frame_count.frame >= *next_step_frame {
                    if let Some((next_position, next_direction)) =
                        get_next_position_direction(*position, *direction)
                    {
                        *position = next_position;
                        *direction = next_direction;
                        *next_step_frame += FPS / 5;

                        clear_position_and_spawn_wall(*position);

                        None
                    } else {
                        Some(WallOfDeath::Done)
                    }
                } else {
                    None
                }
            }
            WallOfDeath::Done => None,
        };

        if let Some(new_state) = new_state {
            *wall_of_death = new_state;
        } else {
            break;
        }
    }
}

pub fn finish_round(
    mut commands: Commands,
    query: Query<&Player, Without<Dead>>,
    frame_count: Res<FrameCount>,
    game_end_frame: Res<GameEndFrame>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    let round_outcome = if frame_count.frame >= game_end_frame.0 || query.iter().count() == 0 {
        Some(RoundOutcome::Tie)
    } else if let Ok(player) = query.get_single() {
        Some(RoundOutcome::Winner(player.id))
    } else {
        None
    };

    if let Some(round_outcome) = round_outcome {
        commands.insert_resource(GameFreeze {
            end_frame: frame_count.frame + FPS, /* 1 second */
            post_freeze_action: Some(PostFreezeAction::ShowLeaderboard(round_outcome)),
        });
    }
}

pub fn cleanup_dead(
    mut commands: Commands,
    query: Query<(Entity, &Dead)>,
    frame_count: Res<FrameCount>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    for (e, d) in query.iter() {
        if frame_count.frame >= d.cleanup_frame {
            commands.entity(e).despawn_recursive();
        }
    }
}

pub fn show_leaderboard(
    mut session_rng: ResMut<SessionRng>,
    mut commands: Commands,
    game_textures: Res<GameTextures>,
    fonts: Res<Fonts>,
    mut leaderboard: ResMut<Leaderboard>,
    game_freeze: Option<ResMut<GameFreeze>>,
    primary_query: Query<&Window, With<PrimaryWindow>>,
    query: Query<Entity, With<UIRoot>>,
    frame_count: Res<FrameCount>,
) {
    if let Some(GameFreeze {
        end_frame: freeze_end_frame,
        post_freeze_action: Some(PostFreezeAction::ShowLeaderboard(round_outcome)),
    }) = game_freeze.as_deref()
    {
        if frame_count.frame >= *freeze_end_frame {
            let next_action = match round_outcome {
                RoundOutcome::Winner(player_id) => {
                    println!("Round winner: {:?}", player_id.0);
                    let player_score = leaderboard.scores.get_mut(player_id).unwrap();
                    *player_score += 1;

                    if *player_score >= leaderboard.winning_score {
                        PostFreezeAction::ShowTournamentWinner { winner: *player_id }
                    } else {
                        PostFreezeAction::StartNewRound
                    }
                }
                RoundOutcome::Tie => {
                    println!("Tie!");
                    PostFreezeAction::StartNewRound
                }
            };

            commands.entity(query.single()).with_children(|parent| {
                let window = primary_query.get_single().unwrap();

                setup_leaderboard_display(
                    &mut session_rng.0,
                    parent,
                    window.height(),
                    window.width(),
                    &game_textures,
                    &fonts,
                    &leaderboard,
                    *round_outcome,
                );
            });

            commands.insert_resource(GameFreeze {
                end_frame: frame_count.frame + LEADERBOARD_DISPLAY_FRAME_COUNT,
                post_freeze_action: Some(next_action),
            });
        }
    }
}

pub fn show_tournament_winner(
    mut session_rng: ResMut<SessionRng>,
    mut commands: Commands,
    game_freeze: Option<Res<GameFreeze>>,
    frame_count: Res<FrameCount>,
    mut leaderboard: ResMut<Leaderboard>,
    mut world_type: ResMut<WorldType>,
) {
    if let Some(GameFreeze {
        end_frame: game_end_frame,
        post_freeze_action: Some(PostFreezeAction::ShowTournamentWinner { winner }),
    }) = game_freeze.as_deref()
    {
        if frame_count.frame >= *game_end_frame {
            println!("Player {} WINNER WINNER CHICKEN DINNER", winner.0);

            // TODO show winner

            // setup new tournament //

            // reset the leaderboard
            for (_, score) in &mut leaderboard.scores {
                *score = 0;
            }

            // choose a world for the next tournament
            *world_type = world_type.next_random(&mut session_rng.0);

            commands.insert_resource(GameFreeze {
                end_frame: frame_count.frame + TOURNAMENT_WINNER_DISPLAY_FRAME_COUNT,
                post_freeze_action: Some(PostFreezeAction::StartNewRound),
            })
        }
    }
}

pub fn start_new_round(
    mut session_rng: ResMut<SessionRng>,
    mut commands: Commands,
    game_freeze: Option<Res<GameFreeze>>,
    frame_count: Res<FrameCount>,
    query: Query<Entity, (Without<Window>, Without<Camera2d>)>,
    map_size: Res<MapSize>,
    world_type: Res<WorldType>,
    matchbox_config: Res<MatchboxConfig>,
    game_textures: ResMut<GameTextures>,
    fonts: Res<Fonts>,
    hud_colors: Res<HUDColors>,
) {
    if let Some(GameFreeze {
        end_frame: freeze_end_frame,
        post_freeze_action: Some(PostFreezeAction::StartNewRound),
    }) = game_freeze.as_deref()
    {
        if frame_count.frame >= *freeze_end_frame {
            // clear game state
            for e in query.iter() {
                // TODO should everything be rollbackable now?
                commands.entity(e).despawn();
            }

            let round_start_frame = frame_count.frame + GAME_START_FREEZE_FRAME_COUNT;
            setup_round(
                &mut session_rng.0,
                &mut commands,
                *map_size,
                *world_type,
                &game_textures,
                &fonts,
                &hud_colors,
                matchbox_config.number_of_players,
                round_start_frame,
            );
            commands.insert_resource(GameFreeze {
                end_frame: round_start_frame,
                post_freeze_action: None,
            })
        }
    }
}

pub fn finish_actionless_game_freeze(
    mut commands: Commands,
    game_freeze: Option<Res<GameFreeze>>,
    frame_count: Res<FrameCount>,
) {
    if let Some(GameFreeze {
        end_frame: freeze_end_frame,
        post_freeze_action: None,
    }) = game_freeze.as_deref()
    {
        if frame_count.frame >= *freeze_end_frame {
            commands.remove_resource::<GameFreeze>();
        }
    }
}
