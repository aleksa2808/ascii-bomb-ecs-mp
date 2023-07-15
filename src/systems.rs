use bevy::{prelude::*, utils::HashSet, window::PrimaryWindow};
use bevy_ggrs::{
    ggrs::{PlayerHandle, SessionBuilder},
    AddRollbackCommandExtension, PlayerInputs, Session,
};
use bevy_matchbox::{
    prelude::{PeerState, SingleChannel},
    MatchboxSocket,
};

use crate::{
    components::*,
    constants::{
        COLORS, FPS, HUD_HEIGHT, INPUT_ACTION, INPUT_DOWN, INPUT_LEFT, INPUT_RIGHT, INPUT_UP,
        PIXEL_SCALE, TILE_HEIGHT, TILE_WIDTH,
    },
    resources::*,
    types::{Direction, PlayerInput},
    utils::{get_x, get_y, init_hud, spawn_map},
    AppState, GGRSConfig,
};

pub fn start_matchbox_socket(mut commands: Commands, args: Res<Args>) {
    let room_id = match &args.room {
        Some(id) => id.clone(),
        None => format!("bevy_ggrs?next={}", &args.players),
    };

    let room_url = format!("{}/{}", &args.matchbox, room_id);
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
    args: Res<Args>,
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
    let remaining = args.players - (connected_peers + 1);
    query.single_mut().sections[0].value = format!("Waiting for {remaining} more player(s)",);
    if remaining > 0 {
        return;
    }

    info!("All peers have joined, going in-game");

    // extract final player list
    let players = socket.players();

    let max_prediction = 12;

    // create a GGRS P2P session
    let mut sess_build = SessionBuilder::<GGRSConfig>::new()
        .with_num_players(args.players)
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

    // transition to in-game state
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

    let mut spawn_player = |penguin_tag: Penguin| {
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
    args: Res<Args>,
) {
    let world_id = WorldID(1);
    game_textures.set_map_textures(world_id);

    let (map_size, percent_of_passable_positions_to_fill) =
        get_battle_mode_map_size_fill(args.players);

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

    let players: Vec<Penguin> = (0..args.players).map(Penguin).collect();

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
    inputs: Res<PlayerInputs<GGRSConfig>>,
    mut p: ParamSet<(
        Query<(&mut Transform, &Penguin, &mut Position, &mut Sprite)>,
        Query<&Position, With<Solid>>,
    )>,
) {
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
    mut query: Query<(Entity, &Penguin, &Position, &mut BombSatchel)>,
    query2: Query<&Position, With<Solid>>,
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

                    parent.spawn((Text2dBundle {
                        text,
                        transform: Transform::from_xyz(0.0, TILE_HEIGHT as f32 / 8.0 * 2.0, 0.0),
                        ..Default::default()
                    },));
                });
        }
    }
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
    // audio: Res<Audio>,
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

    let mut sound_played = false;

    let v: Vec<(Entity, Bomb, Position)> = p
        .p0()
        .iter()
        .filter(|(_, b, _)| frame_count.frame >= b.expiration_frame)
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
