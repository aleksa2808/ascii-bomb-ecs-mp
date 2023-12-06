use bevy::{
    prelude::*,
    utils::{HashMap, HashSet},
    window::PrimaryWindow,
};
use bevy_ggrs::{
    ggrs::{PlayerType, SessionBuilder},
    AddRollbackCommandExtension, PlayerInputs, Rollback, RollbackOrdered, Session,
};
use bevy_matchbox::{
    matchbox_socket::{MultipleChannels, RtcIceServerConfig, WebRtcSocketBuilder},
    prelude::PeerState,
    MatchboxSocket,
};
use itertools::Itertools;
use rand::{
    rngs::StdRng,
    seq::{IteratorRandom, SliceRandom},
    Rng, SeedableRng,
};

use crate::{
    components::*,
    constants::{
        BOMB_SHORTENED_FUSE_FRAME_COUNT, BOMB_Z_LAYER, COLORS, FIRE_Z_LAYER, FPS,
        GAME_START_FREEZE_FRAME_COUNT, GET_READY_DISPLAY_FRAME_COUNT, HUD_HEIGHT, INPUT_ACTION,
        INPUT_DOWN, INPUT_LEFT, INPUT_RIGHT, INPUT_UP, ITEM_SPAWN_CHANCE_PERCENTAGE,
        LEADERBOARD_DISPLAY_FRAME_COUNT, MOVING_OBJECT_FRAME_INTERVAL, PIXEL_SCALE,
        PLAYER_DEATH_FRAME_DELAY, TILE_HEIGHT, TILE_WIDTH, TOURNAMENT_WINNER_DISPLAY_FRAME_COUNT,
        WALL_Z_LAYER,
    },
    resources::*,
    types::{Direction, PlayerID, PostFreezeAction, RoundOutcome},
    utils::{
        burn_item, decode, format_hud_time, generate_item_at_position, get_x, get_y,
        setup_fullscreen_message_display, setup_get_ready_display, setup_leaderboard_display,
        setup_round, setup_tournament_winner_display,
    },
    AppState, GgrsConfig,
};

pub fn print_network_stats_system(
    time: Res<Time>,
    mut timer: ResMut<NetworkStatsTimer>,
    session: Option<Res<Session<GgrsConfig>>>,
) {
    if timer.0.tick(time.delta()).just_finished() {
        if let Some(sess) = session {
            match sess.as_ref() {
                Session::P2P(s) => {
                    let num_players = s.num_players();
                    for i in 0..num_players {
                        if let Ok(stats) = s.network_stats(i) {
                            info!("NetworkStats for player {}: {:?}", i, stats);
                        }
                    }
                }
                _ => unreachable!(),
            }
        }
    }
}

pub fn setup_lobby(
    mut commands: Commands,
    matchbox_config: Res<MatchboxConfig>,
    fonts: Res<Fonts>,
    mut primary_window_query: Query<&mut Window, With<PrimaryWindow>>,
) {
    // choose map size based on player count
    let map_size = if matchbox_config.number_of_players > 4 {
        MapSize {
            rows: 13,
            columns: 17,
        }
    } else {
        MapSize {
            rows: 9,
            columns: 13,
        }
    };
    commands.insert_resource(map_size);

    // resize window based on map size
    let mut window = primary_window_query.single_mut();
    window.resolution.set(
        (map_size.columns * TILE_WIDTH) as f32,
        (HUD_HEIGHT + map_size.rows * TILE_HEIGHT) as f32,
    );

    // spawn the main camera
    commands.spawn(Camera2dBundle {
        transform: Transform::from_xyz(
            ((map_size.columns * TILE_WIDTH) as f32) / 2.0,
            -((map_size.rows * TILE_HEIGHT - HUD_HEIGHT) as f32 / 2.0),
            999.9,
        ),
        ..default()
    });

    setup_fullscreen_message_display(&mut commands, &window, &fonts, "Entering lobby...");
}

pub fn start_matchbox_socket(mut commands: Commands, matchbox_config: Res<MatchboxConfig>) {
    let room_id = match &matchbox_config.room {
        Some(id) => id.clone(),
        None => format!(
            "ascii_bomb_ecs_mp?next={}",
            &matchbox_config.number_of_players
        ),
    };

    let matchbox_server_url = match matchbox_config.matchbox_server_url.clone() {
        Some(url) => url,
        None => "wss://match-0-6.helsing.studio".to_string(),
    };

    let room_url = format!("{}/{}", matchbox_server_url, room_id);
    info!("Connecting to the matchbox server: {room_url:?}");

    let rtc_ice_server_config = match &matchbox_config.ice_server_config {
        Some(config) => RtcIceServerConfig { urls: vec![config.url.clone()], username: config.username.clone(), credential: config.credential.clone() },
        None => RtcIceServerConfig {
            urls: vec![decode("dHVybjpldS10dXJuNy54aXJzeXMuY29tOjM0Nzg/dHJhbnNwb3J0PXVkcA")],
            username: Some(decode("UENMWW5yLWpYZjRZd1VPRDFBR1pxdHVpQzRZeEZFenlJVi10X09LTmxQUG9qbkN6UG5BeXVHVUdDZ2hQTEVfa0FBQUFBR1ZTU21oaGJHVnJjMkV5T0RBNA")),
            credential: Some(decode("MjI0ZDdhZmEtODIzZi0xMWVlLWFlODMtMDI0MmFjMTQwMDA0")),
        },
    };

    commands.insert_resource(MatchboxSocket::from(
        WebRtcSocketBuilder::new(room_url)
            .ice_server(rtc_ice_server_config)
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
    primary_window_query: Query<&Window, With<PrimaryWindow>>,
    mut info_text_query: Query<(&mut Text, &mut Style), With<FullscreenMessageText>>,
) {
    // regularly call update_peers to update the list of connected peers
    for (peer, new_state) in socket.update_peers() {
        // you can also handle the specific dis(connections) as they occur:
        match new_state {
            PeerState::Connected => {
                info!("Peer {peer} connected, sending them our local RNG seed.");

                // send the local RNG seed to peer
                let packet = rng_seeds.local.to_be_bytes().to_vec().into_boxed_slice();
                socket.channel(1).send(packet, peer);

                // reserve a spot for the peer's incoming RNG seed
                rng_seeds.remote.insert(peer, None);
            }
            PeerState::Disconnected => {
                info!("Peer {peer} disconnected.");

                // clear the peer's RNG seed spot
                rng_seeds.remote.remove(&peer);
            }
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

        if let Some(rng_seed) = rng_seeds.remote.get_mut(&peer) {
            assert!(
                rng_seed.is_none(),
                "Received an RNG seed from peer {peer} twice!",
            );
            info!("Received an RNG seed from peer {peer}: {remote_seed}");
            *rng_seed = Some(remote_seed);
        } else {
            info!("Received an RNG seed from a disconnected peer {peer}, discarding...")
        }
    }

    let peer_rng_seeds = rng_seeds.remote.values().filter_map(|r| *r).collect_vec();
    let remaining =
        matchbox_config.number_of_players - (1 /* local player */ + peer_rng_seeds.len());

    // update and recenter the info text
    {
        let message = format!("Waiting for {remaining} more player(s)...");
        let message_length = message.len();
        let (mut text, mut style) = info_text_query.single_mut();
        text.sections[0].value = message;
        style.left = Val::Px(
            primary_window_query.single().width() / 2.0 - (message_length * PIXEL_SCALE) as f32,
        );
    }

    if remaining > 0 {
        return;
    }

    let shared_seed =
        rng_seeds.local ^ peer_rng_seeds.into_iter().reduce(|acc, e| acc ^ e).unwrap();
    info!("Generated the shared RNG seed: {shared_seed}");
    commands.remove_resource::<RngSeeds>();
    commands.insert_resource(SessionRng(StdRng::seed_from_u64(shared_seed)));

    // extract final player list
    let players = socket.players();

    let mut sess_build = SessionBuilder::<GgrsConfig>::new()
        .with_num_players(matchbox_config.number_of_players)
        .with_desync_detection_mode(bevy_ggrs::ggrs::DesyncDetection::On { interval: 1 });

    let mut local_player_id = None;
    for (i, player) in players.into_iter().enumerate() {
        sess_build = sess_build
            .add_player(player, i)
            .expect("failed to add player");

        if let PlayerType::Local = player {
            assert!(local_player_id.is_none());
            info!("Local player ID: {i}");
            local_player_id = Some(LocalPlayerID(i));
        }
    }
    commands.insert_resource(local_player_id.unwrap());

    let channel = socket.take_channel(0).unwrap();

    let sess = sess_build
        .start_p2p_session(channel)
        .expect("failed to start session");

    commands.insert_resource(Session::P2P(sess));

    app_state.set(AppState::InGame);
}

pub fn teardown_lobby(
    teardown_entities_query: Query<Entity, (Without<Window>, Without<Camera2d>)>,
    mut commands: Commands,
) {
    teardown_entities_query
        .iter()
        .for_each(|e| commands.entity(e).despawn());
}

pub fn handle_ggrs_events(
    mut session: ResMut<Session<GgrsConfig>>,
    mut commands: Commands,
    fonts: Res<Fonts>,
    primary_window_query: Query<&Window, With<PrimaryWindow>>,
    teardown_entities_query: Query<Entity, (Without<Window>, Without<Camera2d>)>,
    mut app_state: ResMut<NextState<AppState>>,
) {
    match session.as_mut() {
        Session::P2P(s) => {
            for event in s.events() {
                info!("GgrsEvent: {event:?}");
                let error_message = match event {
                    bevy_ggrs::ggrs::GgrsEvent::Disconnected { .. } => Some("DISCONNECTED!"),
                    bevy_ggrs::ggrs::GgrsEvent::DesyncDetected { .. } => Some("DESYNCED!"),
                    _ => None,
                };

                if let Some(error_message) = error_message {
                    warn!("{}", error_message);
                    commands.remove_resource::<Session<GgrsConfig>>();
                    teardown_entities_query
                        .iter()
                        .for_each(|e| commands.entity(e).despawn());
                    setup_fullscreen_message_display(
                        &mut commands,
                        primary_window_query.single(),
                        &fonts,
                        error_message,
                    );
                    app_state.set(AppState::Error);
                    return;
                }
            }
        }
        _ => unreachable!(),
    }
}

pub fn setup_game(
    mut commands: Commands,
    mut session_rng: ResMut<SessionRng>,
    primary_window_query: Query<&Window, With<PrimaryWindow>>,
    matchbox_config: Res<MatchboxConfig>,
    frame_count: Res<FrameCount>,
    game_textures: Res<GameTextures>,
    fonts: Res<Fonts>,
    local_player_id: Res<LocalPlayerID>,
) {
    // choose the initial world
    let world_type = WorldType::random(&mut session_rng.0);
    commands.insert_resource(world_type);

    // setup the tournament leaderboard
    commands.insert_resource(Leaderboard {
        scores: (0..matchbox_config.number_of_players)
            .map(|p| (PlayerID(p), 0))
            .collect(),
        winning_score: 3,
    });

    // setup the "get ready" display
    setup_get_ready_display(
        &mut commands,
        primary_window_query.single(),
        &game_textures,
        &fonts,
        matchbox_config.number_of_players,
        local_player_id.0,
    );
    commands.remove_resource::<LocalPlayerID>();

    commands.insert_resource(GameFreeze {
        end_frame: frame_count.frame + GET_READY_DISPLAY_FRAME_COUNT,
        post_freeze_action: Some(PostFreezeAction::StartNewRound),
    });
}

pub fn increase_frame_system(mut frame_count: ResMut<FrameCount>) {
    frame_count.frame += 1;
}

pub fn update_hud_clock(
    game_end_frame: Option<Res<GameEndFrame>>,
    mut clock_text_query: Query<&mut Text, With<GameTimerDisplay>>,
    frame_count: Res<FrameCount>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    let game_end_frame = game_end_frame.unwrap();
    let remaining_seconds =
        ((game_end_frame.0 - frame_count.frame) as f32 / FPS as f32).ceil() as usize;
    clock_text_query.single_mut().sections[0].value = format_hud_time(remaining_seconds);
}

pub fn update_player_portraits(
    player_query: Query<&Player>,
    mut portrait_visibility_query: Query<(&mut Visibility, &PlayerPortrait)>,
) {
    let player_ids: HashSet<PlayerID> = player_query.iter().map(|player| player.id).collect();

    for (mut visibility, portrait) in portrait_visibility_query.iter_mut() {
        if player_ids.contains(&portrait.0) {
            *visibility = Visibility::Visible;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

pub fn player_move(
    mut session_rng: ResMut<SessionRng>,
    mut commands: Commands,
    inputs: Res<PlayerInputs<GgrsConfig>>,
    rollback_ordered: Res<RollbackOrdered>,
    mut alive_player_query: Query<
        (
            &Rollback,
            &Player,
            &mut Position,
            &mut Transform,
            &mut Sprite,
        ),
        (Without<Dead>, Without<Solid>),
    >,
    solid_object_query: Query<(Entity, &Position, Option<&Bomb>), With<Solid>>,
    frame_count: Res<FrameCount>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    let mut solids = HashMap::new();
    for (p, b) in solid_object_query
        .iter()
        .map(|(solid_entity, solid_position, optional_bomb)| {
            (*solid_position, optional_bomb.map(|_| solid_entity))
        })
    {
        let previous_item = solids.insert(p, b);

        // there must only be one solid per position
        // if there are multiple bombs on the same position only one would get updated which could lead to a desync
        assert!(
            previous_item.is_none(),
            "Multiple solid objects on position {p:?}!"
        );
    }
    let solids = solids;

    // player sorting is needed to ensure determinism of pushing bombs
    let mut players = alive_player_query
        .iter_mut()
        .sorted_by_cached_key(|q| rollback_ordered.order(*q.0))
        .collect_vec();
    // shuffle to ensure fairness in situations where two players push the same bomb in the same frame
    players.shuffle(&mut session_rng.0);
    for (_, player, mut position, mut transform, mut sprite) in players {
        let input = inputs[player.id.0].0 .0;
        for (input_mask, moving_direction) in [
            (INPUT_UP, Direction::Up),
            (INPUT_DOWN, Direction::Down),
            (INPUT_LEFT, Direction::Left),
            (INPUT_RIGHT, Direction::Right),
        ] {
            if input & input_mask != 0 {
                info!(
                    "[frame:{}] Player {} moved in direction {moving_direction:?} at position: {position:?}",
                    frame_count.frame, player.id.0,
                );

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
                            commands.entity(bomb_entity).insert(Moving {
                                direction: moving_direction,
                                next_move_frame: frame_count.frame,
                                frame_interval: MOVING_OBJECT_FRAME_INTERVAL,
                            });
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
}

pub fn bomb_move(
    mut commands: Commands,
    rollback_ordered: Res<RollbackOrdered>,
    mut position_queries: ParamSet<(
        Query<
            (
                &Rollback,
                Entity,
                &mut Moving,
                &mut Position,
                &mut Transform,
            ),
            With<Bomb>,
        >,
        Query<&Position, (Without<Moving>, Or<(With<Solid>, With<Item>, With<Player>)>)>,
    )>,
    frame_count: Res<FrameCount>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    let mut static_impassable_object_positions: HashSet<Position> = {
        let positions_of_moving_bombs_not_ready_to_move = position_queries
            .p0()
            .iter()
            .filter(|(_, _, m, _, _)| frame_count.frame < m.next_move_frame)
            .map(|(_, _, _, &p, _)| p)
            .collect_vec();

        position_queries
            .p1()
            .iter()
            .copied()
            .chain(positions_of_moving_bombs_not_ready_to_move)
            .collect()
    };
    let mut positions_of_bombs_ready_to_move: HashSet<Position> = position_queries
        .p0()
        .iter()
        .filter(|(_, _, m, _, _)| frame_count.frame >= m.next_move_frame)
        .map(|(_, _, _, &p, _)| p)
        .collect();

    // all moving bombs that are ready to move this frame
    // bomb sorting is needed to ensure movement determinism
    let mut tmp = position_queries.p0();
    let mut moving_bombs_left_to_check = tmp
        .iter_mut()
        .filter(|(_, _, moving, _, _)| frame_count.frame >= moving.next_move_frame)
        .sorted_by_cached_key(|q| rollback_ordered.order(*q.0))
        .collect_vec();

    loop {
        let moving_bombs = moving_bombs_left_to_check;
        moving_bombs_left_to_check = vec![];
        let mut bombs_moved = false;

        for moving_bomb in moving_bombs {
            let moving_bomb_entity = moving_bomb.1;
            let current_position = *moving_bomb.3;
            let next_position = current_position.offset(moving_bomb.2.direction, 1);
            if static_impassable_object_positions.contains(&next_position) {
                // hit an impassable object, stop moving the bomb
                commands.entity(moving_bomb_entity).remove::<Moving>();
                positions_of_bombs_ready_to_move.remove(&current_position);
                static_impassable_object_positions.insert(current_position);
            } else if positions_of_bombs_ready_to_move.contains(&next_position) {
                // a bomb that is about to move is blocking the way, check again later
                moving_bombs_left_to_check.push(moving_bomb);
            } else {
                // the way is clear, move the bomb
                let (_, _, mut moving, mut position, mut transform) = moving_bomb;

                *position = next_position;

                let translation = &mut transform.translation;
                translation.x = get_x(position.x);
                translation.y = get_y(position.y);

                moving.next_move_frame += moving.frame_interval;

                positions_of_bombs_ready_to_move.remove(&current_position);
                static_impassable_object_positions.insert(next_position);
                bombs_moved = true;
            }
        }

        // stop iterating if there are no more objects which should move this frame or if we couldn't move any more objects in this iteration (objects trying to move into one another)
        if moving_bombs_left_to_check.is_empty() || !bombs_moved {
            break;
        }
    }
}

pub fn pick_up_item(
    mut commands: Commands,
    game_textures: Res<GameTextures>,
    mut alive_player_query: Query<(&mut Player, &Position, &mut BombSatchel), Without<Dead>>,
    mut item_query: Query<(Entity, &Item, &Position, &mut Handle<Image>)>,
    frame_count: Res<FrameCount>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    for (item_entity, &item, &item_position, mut item_texture) in item_query.iter_mut() {
        let mut players_at_item_position =
            alive_player_query
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
                info!(
                    "[frame:{}] Player {} picked up {:?} at position: {item_position:?}",
                    frame_count.frame, player.id.0, item,
                );
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
                info!("[frame:{}] Multiple players arrived at item position ({item_position:?}) at the same time! In the ensuing chaos the item was destroyed...", frame_count.frame);
                burn_item(
                    &mut commands,
                    &game_textures,
                    item_entity,
                    &mut item_texture,
                    frame_count.frame,
                );
            }
            (None, Some(_)) => unreachable!(),
        }
    }
}

pub fn bomb_drop(
    mut session_rng: ResMut<SessionRng>,
    mut commands: Commands,
    inputs: Res<PlayerInputs<GgrsConfig>>,
    game_textures: Res<GameTextures>,
    fonts: Res<Fonts>,
    world_type: Res<WorldType>,
    rollback_ordered: Res<RollbackOrdered>,
    mut alive_player_query: Query<(&Rollback, &Player, &Position, &mut BombSatchel), Without<Dead>>,
    invalid_bomb_position_query: Query<&Position, Or<(With<Solid>, With<BurningItem>)>>,
    frame_count: Res<FrameCount>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    let mut invalid_bomb_positions: HashSet<Position> =
        invalid_bomb_position_query.iter().copied().collect();

    // player sorting is needed to ensure determinism of spawning bombs
    let mut players = alive_player_query
        .iter_mut()
        .sorted_by_cached_key(|q| rollback_ordered.order(*q.0))
        .collect_vec();
    // shuffle to ensure fairness in situations where two players try to place a bomb in the same frame
    players.shuffle(&mut session_rng.0);
    for (_, player, position, mut bomb_satchel) in players {
        if inputs[player.id.0].0 .0 & INPUT_ACTION != 0
            && bomb_satchel.bombs_available > 0
            && !invalid_bomb_positions.contains(position)
        {
            info!(
                "[frame:{}] Player {} placed a bomb at position: {:?}",
                frame_count.frame, player.id.0, position
            );
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

            invalid_bomb_positions.insert(*position);
        }
    }
}

pub fn animate_fuse(
    frame_count: Res<FrameCount>,
    fonts: Res<Fonts>,
    bomb_query: Query<&Bomb>,
    mut fuse_query: Query<(&Parent, &mut Text, &Fuse, &mut Transform)>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    for (parent, mut text, fuse, mut transform) in fuse_query.iter_mut() {
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

        let bomb = bomb_query.get(parent.get()).unwrap();
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
    fire_query: Query<(Entity, &Fire)>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    for (entity, fire) in fire_query.iter() {
        if frame_count.frame >= fire.expiration_frame {
            commands.entity(entity).despawn_recursive();
        }
    }
}

pub fn crumbling_tick(
    mut commands: Commands,
    mut session_rng: ResMut<SessionRng>,
    frame_count: Res<FrameCount>,
    crumbling_query: Query<(Entity, &Crumbling, &Position)>,
    game_textures: Res<GameTextures>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    for (entity, _, position) in crumbling_query
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
    burning_item_query: Query<(Entity, &BurningItem)>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    for (entity, _) in burning_item_query
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
    rollback_ordered: Res<RollbackOrdered>,
    mut position_queries: ParamSet<(
        Query<(&Rollback, Entity, &mut Bomb, &Position)>,
        Query<(Entity, &Position, Option<&Bomb>), With<Solid>>,
    )>,
    mut alive_player_query: Query<(&Player, &mut BombSatchel), Without<Dead>>,
    mut destructible_wall_query: Query<
        (Entity, &Position, &mut Handle<Image>, Option<&Crumbling>),
        (With<Wall>, With<Destructible>),
    >,
    fire_query: Query<(&Rollback, Entity, &Position), With<Fire>>,
    frame_count: Res<FrameCount>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    let fireproof_positions: HashSet<Position> = position_queries
        .p1()
        .iter()
        .filter_map(|(_, p, b)| {
            // ignore bombs that are currently exploding
            if !matches!(b, Some(b) if  frame_count.frame >= b.expiration_frame) {
                Some(p)
            } else {
                None
            }
        })
        .copied()
        .collect();

    let mut fire_touched_positions = HashSet::new();
    let spawn_fire = |commands: &mut Commands, position: Position| {
        // remove previous fire at position if it exists
        for (_, e, _) in fire_query.iter().filter(|(_, _, &p)| p == position) {
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

    // sorting is needed to ensure fire spawn determinism
    let tmp = position_queries.p0();
    let exploding_bombs = tmp
        .iter()
        .filter(|(_, _, b, _)| frame_count.frame >= b.expiration_frame)
        .sorted_by_cached_key(|q| rollback_ordered.order(*q.0))
        .map(|(_, e, &b, &p)| (e, b, p))
        .collect_vec();
    for (entity, bomb, position) in exploding_bombs {
        commands.entity(entity).despawn_recursive();

        if let Some(owner) = bomb.owner {
            if let Some((_, mut bomb_satchel)) = alive_player_query
                .iter_mut()
                .find(|(player, _)| player.id == owner)
            {
                bomb_satchel.bombs_available += 1;
            }
        }

        if !fire_touched_positions.contains(&position) {
            spawn_fire(&mut commands, position);
            fire_touched_positions.insert(position);
        }
        for direction in Direction::LIST {
            for position in (1..=bomb.range).map(|i| position.offset(direction, i)) {
                if fireproof_positions.contains(&position) {
                    if !fire_touched_positions.contains(&position) {
                        // bomb burn
                        position_queries
                            .p0()
                            .iter_mut()
                            .filter(|(_, _, _, &bomb_position)| bomb_position == position)
                            .for_each(|(_, _, mut bomb, _)| {
                                bomb.expiration_frame = bomb
                                    .expiration_frame
                                    .min(frame_count.frame + BOMB_SHORTENED_FUSE_FRAME_COUNT);
                            });

                        // destructible wall burn
                        destructible_wall_query
                            .iter_mut()
                            .filter(|(_, &destructible_wall_position, _, crumbling)| {
                                destructible_wall_position == position && crumbling.is_none()
                            })
                            .for_each(|(entity, _, mut texture, _)| {
                                commands.entity(entity).insert(Crumbling {
                                    expiration_frame: frame_count.frame + FPS / 2,
                                });
                                *texture = game_textures
                                    .get_map_textures(*world_type)
                                    .burning_wall
                                    .clone();
                            });

                        fire_touched_positions.insert(position);
                    }
                    break;
                }

                if !fire_touched_positions.contains(&position) {
                    spawn_fire(&mut commands, position);
                    fire_touched_positions.insert(position);
                }
            }
        }
    }
}

pub fn player_burn(
    mut commands: Commands,
    fire_query: Query<&Position, With<Fire>>,
    alive_player_query: Query<(Entity, &Player, &Position), Without<Dead>>,
    frame_count: Res<FrameCount>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    let fire_positions: HashSet<Position> = fire_query.iter().copied().collect();
    alive_player_query
        .iter()
        .filter(|(_, _, position)| fire_positions.contains(*position))
        .for_each(|(entity, player, position)| {
            info!(
                "[frame:{}] Player {} was burned at position: {position:?}",
                frame_count.frame, player.id.0
            );
            commands.entity(entity).insert(Dead {
                cleanup_frame: frame_count.frame + PLAYER_DEATH_FRAME_DELAY,
            });
        });
}

pub fn bomb_burn(
    fire_query: Query<&Position, With<Fire>>,
    mut bomb_query: Query<(&mut Bomb, &Position)>,
    frame_count: Res<FrameCount>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    let fire_positions: HashSet<Position> = fire_query.iter().copied().collect();
    bomb_query
        .iter_mut()
        .filter(|(_, position)| fire_positions.contains(*position))
        .for_each(|(mut bomb, _)| {
            bomb.expiration_frame = bomb
                .expiration_frame
                .min(frame_count.frame + BOMB_SHORTENED_FUSE_FRAME_COUNT);
        });
}

pub fn item_burn(
    mut commands: Commands,
    game_textures: Res<GameTextures>,
    fire_query: Query<&Position, With<Fire>>,
    mut item_query: Query<(Entity, &Position, &mut Handle<Image>), With<Item>>,
    frame_count: Res<FrameCount>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    let fire_positions: HashSet<Position> = fire_query.iter().copied().collect();
    item_query
        .iter_mut()
        .filter(|(_, position, _)| fire_positions.contains(*position))
        .for_each(|(entity, _, mut texture)| {
            burn_item(
                &mut commands,
                &game_textures,
                entity,
                &mut texture,
                frame_count.frame,
            );
        });
}

pub fn wall_of_death_update(
    mut commands: Commands,
    game_textures: Res<GameTextures>,
    wall_of_death: Option<ResMut<WallOfDeath>>,
    world_type: Res<WorldType>,
    map_size: Res<MapSize>,
    indestructible_wall_query: Query<&Position, (With<Wall>, Without<Destructible>)>,
    entity_query: Query<(Entity, &Position, Option<&Bomb>)>,
    mut player_query: Query<(&Player, &mut BombSatchel, Option<&Dead>)>,
    frame_count: Res<FrameCount>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    let mut wall_of_death = wall_of_death.unwrap();

    let get_next_position_direction = |mut position: Position,
                                       mut direction: Direction|
     -> Option<(Position, Direction)> {
        let end_position = Position {
            y: map_size.rows as isize - 3,
            x: 3,
        };

        let indestructible_walls: HashSet<Position> =
            indestructible_wall_query.iter().copied().collect();
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
            if !indestructible_walls.contains(&position) {
                break Some((position, direction));
            }
        }
    };

    let mut clear_position_and_spawn_wall = |position: Position| {
        for (entity, position, bomb) in entity_query.iter().filter(|(_, &p, _)| p == position) {
            if let Ok((player, _, dead)) = player_query.get(entity) {
                if dead.is_none() {
                    info!(
                        "[frame:{}] Player {} was crushed at position: {position:?}",
                        frame_count.frame, player.id.0
                    );
                    commands.entity(entity).insert(Dead {
                        cleanup_frame: frame_count.frame + PLAYER_DEATH_FRAME_DELAY,
                    });
                }
            } else {
                commands.entity(entity).despawn_recursive();

                if let Some(&Bomb {
                    owner: Some(owner), ..
                }) = bomb
                {
                    if let Some((_, mut bomb_satchel, _)) = player_query
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
                    info!("[frame:{}] Wall of Death activated!", frame_count.frame);

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

pub fn cleanup_dead(
    mut session_rng: ResMut<SessionRng>,
    mut commands: Commands,
    dead_entity_query: Query<(Entity, &Dead)>,
    invalid_item_position_query: Query<
        &Position,
        Or<(
            With<Player>,
            With<Solid>,
            With<Fire>,
            With<BurningItem>,
            With<Item>,
        )>,
    >,
    frame_count: Res<FrameCount>,
    game_textures: Res<GameTextures>,
    map_size: Res<MapSize>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    for (e, d) in dead_entity_query.iter() {
        if frame_count.frame >= d.cleanup_frame {
            commands.entity(e).despawn_recursive();

            // death pinata
            let invalid_item_positions: HashSet<Position> =
                invalid_item_position_query.iter().copied().collect();
            let valid_positions = (1..map_size.rows - 1)
                .flat_map(|y| {
                    (1..map_size.columns - 1).map(move |x| Position {
                        y: y as isize,
                        x: x as isize,
                    })
                })
                .filter(|position| !invalid_item_positions.contains(position));
            for position in valid_positions.choose_multiple(&mut session_rng.0, 3) {
                generate_item_at_position(
                    &mut session_rng.0,
                    &mut commands,
                    &game_textures,
                    position,
                );
            }
        }
    }
}

pub fn check_game_rules(
    solid_object_query: Query<&Position, With<Solid>>,
    fire_query: Query<&Position, With<Fire>>,
    item_query: Query<&Position, With<Item>>,
) {
    for position in solid_object_query.iter().duplicates() {
        warn!("Multiple solid objects at position: {position:?}");
    }
    for position in fire_query.iter().duplicates() {
        warn!("Multiple fires at position: {position:?}");
    }
    for position in item_query.iter().duplicates() {
        warn!("Multiple items at position: {position:?}");
    }
}

pub fn finish_round(
    mut commands: Commands,
    alive_player_query: Query<&Player, Without<Dead>>,
    frame_count: Res<FrameCount>,
    game_end_frame: Option<Res<GameEndFrame>>,
    game_freeze: Option<Res<GameFreeze>>,
) {
    if game_freeze.is_some() {
        return;
    }

    let game_end_frame = game_end_frame.unwrap();

    let round_outcome =
        if frame_count.frame >= game_end_frame.0 || alive_player_query.iter().count() == 0 {
            Some(RoundOutcome::Tie)
        } else if let Ok(player) = alive_player_query.get_single() {
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

pub fn show_leaderboard(
    mut session_rng: ResMut<SessionRng>,
    mut commands: Commands,
    game_textures: Res<GameTextures>,
    fonts: Res<Fonts>,
    mut leaderboard: ResMut<Leaderboard>,
    game_freeze: Option<ResMut<GameFreeze>>,
    primary_window_query: Query<&Window, With<PrimaryWindow>>,
    ui_root_query: Query<Entity, With<UIRoot>>,
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
                    info!("Player {} won the round!", player_id.0);
                    let player_score = leaderboard.scores.get_mut(player_id).unwrap();
                    *player_score += 1;

                    if *player_score >= leaderboard.winning_score {
                        PostFreezeAction::ShowTournamentWinner { winner: *player_id }
                    } else {
                        PostFreezeAction::StartNewRound
                    }
                }
                RoundOutcome::Tie => {
                    info!("The round was a tie!");
                    PostFreezeAction::StartNewRound
                }
            };

            commands
                .entity(ui_root_query.single())
                .with_children(|parent| {
                    let window = primary_window_query.get_single().unwrap();

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
    game_textures: Res<GameTextures>,
    fonts: Res<Fonts>,
    mut world_type: ResMut<WorldType>,
    primary_window_query: Query<&Window, With<PrimaryWindow>>,
    leaderboard_ui_content_query: Query<Entity, With<LeaderboardUIContent>>,
) {
    if let Some(GameFreeze {
        end_frame: freeze_end_frame,
        post_freeze_action: Some(PostFreezeAction::ShowTournamentWinner { winner }),
    }) = game_freeze.as_deref()
    {
        if frame_count.frame >= *freeze_end_frame {
            info!("Player {} won the tournament!", winner.0);

            // clear the leaderboard display and setup the tournament winner display
            commands
                .entity(leaderboard_ui_content_query.single())
                .despawn_descendants()
                .with_children(|parent| {
                    let window = primary_window_query.get_single().unwrap();
                    setup_tournament_winner_display(
                        parent,
                        window.height(),
                        window.width(),
                        &game_textures,
                        &fonts,
                        *winner,
                    );
                });

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
    teardown_entities_query: Query<Entity, (Without<Window>, Without<Camera2d>)>,
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
            for e in teardown_entities_query.iter() {
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
