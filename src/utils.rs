use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine};
use bevy::{
    asset::Handle,
    ecs::entity::Entity,
    prelude::{BuildChildren, ChildBuilder, Commands, NodeBundle, TextBundle, Transform, Vec2},
    render::{color::Color, texture::Image},
    sprite::{Sprite, SpriteBundle},
    text::{Text, TextStyle},
    ui::{node_bundles::ImageBundle, PositionType, Style, UiRect, Val},
    utils::HashSet,
    window::Window,
};
use bevy_ggrs::AddRollbackCommandExtension;
use itertools::Itertools;

use crate::{
    components::{
        BombSatchel, BurningItem, Destructible, FullscreenMessageText, GameTimerDisplay, HUDRoot,
        Item, LeaderboardUIContent, LeaderboardUIRoot, NetworkStatsDisplay, Player, PlayerPortrait,
        PlayerPortraitDisplay, Position, Solid, UIComponent, UIRoot, Wall,
    },
    constants::{
        COLORS, DESTRUCTIBLE_WALL_Z_LAYER, FPS, HUD_HEIGHT, ITEM_Z_LAYER, PIXEL_SCALE,
        PLAYER_Z_LAYER, ROUND_DURATION_SECS, TILE_HEIGHT, TILE_WIDTH, WALL_Z_LAYER,
    },
    resources::{
        Fonts, GameEndFrame, GameTextures, HUDColors, Leaderboard, MapSize, SessionRng,
        WallOfDeath, WorldType,
    },
    types::{Direction, PlayerID, RoundOutcome},
};

pub fn get_x(x: u8) -> f32 {
    TILE_WIDTH as f32 / 2.0 + (x as u32 * TILE_WIDTH) as f32
}

pub fn get_y(y: u8) -> f32 {
    -(TILE_HEIGHT as f32 / 2.0 + (y as u32 * TILE_HEIGHT) as f32)
}

pub fn decode(input: &str) -> String {
    String::from_utf8(STANDARD_NO_PAD.decode(input).unwrap()).unwrap()
}

pub fn shuffle<T>(elements: &mut [T], rng: &mut SessionRng) {
    for i in (1..elements.len()).rev() {
        elements.swap(i, (rng.gen_u64() % (i as u64 + 1)) as usize);
    }
}

pub fn setup_fullscreen_message_display(
    commands: &mut Commands,
    window: &Window,
    fonts: &Fonts,
    message: &str,
) {
    let center_y = window.height() / 2.0 - (4 * PIXEL_SCALE) as f32 /* accounting for the get ready text */;
    let center_x = window.width() / 2.0;

    commands
        .spawn((NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..Default::default()
            },
            background_color: COLORS[0].into(),
            ..Default::default()
        },))
        .with_children(|parent| {
            parent.spawn((
                TextBundle {
                    text: Text::from_section(
                        message,
                        TextStyle {
                            font: fonts.mono.clone(),
                            font_size: 4.0 * PIXEL_SCALE as f32,
                            color: COLORS[15].into(),
                        },
                    ),
                    style: Style {
                        position_type: PositionType::Absolute,
                        top: Val::Px(center_y),
                        left: Val::Px(center_x - (message.len() * PIXEL_SCALE as usize) as f32),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                FullscreenMessageText,
            ));
        });
}

pub fn setup_get_ready_display(
    commands: &mut Commands,
    window: &Window,
    game_textures: &GameTextures,
    fonts: &Fonts,
    number_of_players: u8,
    local_player_id: u8,
) {
    let portrait_distance = (12 - number_of_players) as u32 * PIXEL_SCALE;
    let total_width = number_of_players as u32 * (TILE_WIDTH + 2 * PIXEL_SCALE/* border */)
        + (number_of_players - 1) as u32 * portrait_distance;

    let center_y = window.height() / 2.0 - (4 * PIXEL_SCALE) as f32 /* accounting for the get ready text */;
    let center_x = window.width() / 2.0;
    let offset_x = center_x - total_width as f32 / 2.0;

    commands
        .spawn((NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..Default::default()
            },
            background_color: COLORS[0].into(),
            ..Default::default()
        },))
        .with_children(|parent| {
            for i in 0..number_of_players {
                // highlight the local player
                let border_color = COLORS[if i == local_player_id { 12 } else { 0 }];
                let offset_x = offset_x
                    + (i as u32 * (TILE_WIDTH + 2 * PIXEL_SCALE + portrait_distance)) as f32;

                parent
                    .spawn(NodeBundle {
                        style: Style {
                            position_type: PositionType::Absolute,
                            top: Val::Px(center_y - TILE_HEIGHT as f32 / 2.0),
                            left: Val::Px(offset_x),
                            width: Val::Px(8.0 * PIXEL_SCALE as f32),
                            height: Val::Px(10.0 * PIXEL_SCALE as f32),
                            border: UiRect {
                                left: Val::Px(PIXEL_SCALE as f32),
                                top: Val::Px(PIXEL_SCALE as f32),
                                right: Val::Px(PIXEL_SCALE as f32),
                                bottom: Val::Px(PIXEL_SCALE as f32),
                            },
                            ..Default::default()
                        },
                        background_color: border_color.into(),
                        ..Default::default()
                    })
                    .with_children(|parent| {
                        parent
                            .spawn(NodeBundle {
                                style: Style {
                                    width: Val::Percent(100.0),
                                    height: Val::Percent(100.0),
                                    ..Default::default()
                                },
                                background_color: COLORS[2].into(),
                                ..Default::default()
                            })
                            .with_children(|parent| {
                                parent.spawn(ImageBundle {
                                    style: Style {
                                        width: Val::Percent(100.0),
                                        height: Val::Percent(100.0),
                                        ..Default::default()
                                    },
                                    image: game_textures
                                        .get_player_texture(PlayerID(i))
                                        .clone()
                                        .into(),
                                    ..Default::default()
                                });
                            });
                    });
            }

            parent.spawn(TextBundle {
                text: Text::from_section(
                    "GET READY!",
                    TextStyle {
                        font: fonts.mono.clone(),
                        font_size: 2.0 * PIXEL_SCALE as f32,
                        color: COLORS[15].into(),
                    },
                ),
                style: Style {
                    position_type: PositionType::Absolute,
                    top: Val::Px(center_y + (TILE_WIDTH / 2 + 6 * PIXEL_SCALE) as f32),
                    left: Val::Px(center_x - 5.0 * PIXEL_SCALE as f32),
                    ..Default::default()
                },
                ..Default::default()
            });
        });
}

pub fn format_hud_time(remaining_seconds: u32) -> String {
    format!(
        "{:02}:{:02}",
        remaining_seconds / 60,
        remaining_seconds % 60
    )
}

fn init_hud(
    parent: &mut ChildBuilder,
    hud_colors: &HUDColors,
    fonts: &Fonts,
    width: f32,
    world_type: WorldType,
    game_textures: &GameTextures,
    player_ids: &[PlayerID],
) {
    parent
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Px(width),
                    height: Val::Px(HUD_HEIGHT as f32),
                    ..Default::default()
                },
                background_color: hud_colors.get_background_color(world_type).into(),
                ..Default::default()
            },
            UIComponent,
            HUDRoot,
            PlayerPortraitDisplay,
        ))
        .with_children(|parent| {
            // clock
            parent
                .spawn((
                    NodeBundle {
                        style: Style {
                            position_type: PositionType::Absolute,
                            left: Val::Px(width / 2.0 - 3.0 * PIXEL_SCALE as f32),
                            top: Val::Px(12.0 * PIXEL_SCALE as f32),
                            width: Val::Px(5.0 * PIXEL_SCALE as f32),
                            height: Val::Px(2.0 * PIXEL_SCALE as f32),
                            ..Default::default()
                        },
                        background_color: hud_colors.black_color.into(),
                        ..Default::default()
                    },
                    UIComponent,
                ))
                .with_children(|parent| {
                    parent.spawn((
                        TextBundle {
                            text: Text::from_section(
                                // TODO this is here because the ggrs systems don't seem to start immediately, so the timer has a visual issue; investigate why
                                format_hud_time(ROUND_DURATION_SECS),
                                TextStyle {
                                    font: fonts.mono.clone(),
                                    font_size: 2.0 * PIXEL_SCALE as f32,
                                    color: COLORS[15].into(),
                                },
                            ),
                            style: Style {
                                position_type: PositionType::Absolute,
                                top: Val::Px(0.0),
                                left: Val::Px(0.0),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        UIComponent,
                        GameTimerDisplay,
                    ));
                });

            // network stats
            parent
                .spawn((
                    NodeBundle {
                        style: Style {
                            position_type: PositionType::Absolute,
                            left: Val::Px(width - 6.0 * PIXEL_SCALE as f32),
                            top: Val::Px(0.0),
                            width: Val::Px(6.0 * PIXEL_SCALE as f32),
                            height: Val::Px(
                                2.0 * ((1 + player_ids.len()) * PIXEL_SCALE as usize) as f32,
                            ),
                            ..Default::default()
                        },
                        background_color: hud_colors.black_color.into(),
                        ..Default::default()
                    },
                    UIComponent,
                ))
                .with_children(|parent| {
                    parent.spawn((
                        TextBundle {
                            text: Text::from_section(
                                "ping",
                                TextStyle {
                                    font: fonts.mono.clone(),
                                    font_size: 2.0 * PIXEL_SCALE as f32,
                                    color: COLORS[15].into(),
                                },
                            ),
                            style: Style {
                                position_type: PositionType::Absolute,
                                top: Val::Px(0.0),
                                left: Val::Px(PIXEL_SCALE as f32),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        UIComponent,
                    ));

                    parent.spawn((
                        TextBundle {
                            text: Text::from_section(
                                "",
                                TextStyle {
                                    font: fonts.mono.clone(),
                                    font_size: 2.0 * PIXEL_SCALE as f32,
                                    color: COLORS[15].into(),
                                },
                            ),
                            style: Style {
                                position_type: PositionType::Absolute,
                                top: Val::Px(2.0 * PIXEL_SCALE as f32),
                                left: Val::Px(0.0),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        UIComponent,
                        NetworkStatsDisplay,
                    ));
                });

            // player portraits
            for &player_id in player_ids {
                parent
                    .spawn((
                        NodeBundle {
                            style: Style {
                                position_type: PositionType::Absolute,
                                left: Val::Px(((5 + 12 * player_id.0) as u32 * PIXEL_SCALE) as f32),
                                top: Val::Px(PIXEL_SCALE as f32),
                                width: Val::Px(8.0 * PIXEL_SCALE as f32),
                                height: Val::Px(10.0 * PIXEL_SCALE as f32),
                                border: UiRect {
                                    left: Val::Px(PIXEL_SCALE as f32),
                                    top: Val::Px(PIXEL_SCALE as f32),
                                    right: Val::Px(PIXEL_SCALE as f32),
                                    bottom: Val::Px(PIXEL_SCALE as f32),
                                },
                                ..Default::default()
                            },
                            background_color: hud_colors.portrait_border_color.into(),
                            ..Default::default()
                        },
                        PlayerPortrait(player_id),
                        UIComponent,
                    ))
                    .with_children(|parent| {
                        parent
                            .spawn((
                                NodeBundle {
                                    style: Style {
                                        width: Val::Percent(100.0),
                                        height: Val::Percent(100.0),
                                        ..Default::default()
                                    },
                                    background_color: hud_colors.portrait_background_color.into(),
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
                                            .get_player_texture(player_id)
                                            .clone()
                                            .into(),
                                        ..Default::default()
                                    },
                                    UIComponent,
                                ));
                            });
                    });
            }
        });
}

fn spawn_map(
    rng: &mut SessionRng,
    commands: &mut Commands,
    game_textures: &GameTextures,
    world_type: WorldType,
    map_size: MapSize,
    player_spawn_positions: &[Position],
) {
    // place empty/passable tiles
    for j in 0..map_size.rows {
        for i in 0..map_size.columns {
            commands.spawn(SpriteBundle {
                texture: game_textures.get_map_textures(world_type).empty.clone(),
                transform: Transform::from_xyz(get_x(i), get_y(j), 0.0),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(TILE_WIDTH as f32, TILE_HEIGHT as f32)),
                    ..Default::default()
                },
                ..Default::default()
            });
        }
    }

    // spawn walls
    let mut stone_wall_positions = HashSet::new();
    for i in 0..map_size.rows {
        // left
        stone_wall_positions.insert(Position { y: i, x: 0 });
        // right
        stone_wall_positions.insert(Position {
            y: i,
            x: (map_size.columns - 1),
        });
    }
    for i in 1..map_size.columns - 1 {
        // top
        stone_wall_positions.insert(Position { y: 0, x: i });
        // bottom
        stone_wall_positions.insert(Position {
            y: (map_size.rows - 1),
            x: i,
        });
    }
    // checkered middle
    for i in (2..map_size.rows).step_by(2) {
        for j in (2..map_size.columns).step_by(2) {
            stone_wall_positions.insert(Position { y: i, x: j });
        }
    }

    for position in stone_wall_positions.iter().cloned() {
        commands.spawn((
            SpriteBundle {
                texture: game_textures.get_map_textures(world_type).wall.clone(),
                transform: Transform::from_xyz(get_x(position.x), get_y(position.y), WALL_Z_LAYER),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(TILE_WIDTH as f32, TILE_HEIGHT as f32)),
                    ..Default::default()
                },
                ..Default::default()
            },
            Wall,
            Solid,
            position,
        ));
    }

    let mut destructible_wall_potential_positions: HashSet<Position> = (0..map_size.rows)
        .flat_map(|y| (0..map_size.columns).map(move |x| Position { y, x }))
        .filter(|p| !stone_wall_positions.contains(p))
        .collect();

    let number_of_passable_positions = destructible_wall_potential_positions.len();

    // reserve room for the players (cross-shaped)
    for player_spawn_position in player_spawn_positions {
        destructible_wall_potential_positions.remove(player_spawn_position);
        for position in Direction::LIST
            .iter()
            .map(|direction| player_spawn_position.offset(*direction, 1))
        {
            destructible_wall_potential_positions.remove(&position);
        }
    }

    let number_of_players = player_spawn_positions.len();
    let num_of_destructible_walls_to_place = match number_of_players {
        2..=3 => number_of_passable_positions / 5 * 2,
        4..=8 => number_of_passable_positions / 2,
        _ => unreachable!(),
    };
    if destructible_wall_potential_positions.len() < num_of_destructible_walls_to_place {
        panic!(
            "Not enough passable positions available for placing destructible walls. Have {}, but need at least {}",
            destructible_wall_potential_positions.len(),
            num_of_destructible_walls_to_place
        );
    }

    let mut destructible_wall_positions = destructible_wall_potential_positions
        .into_iter()
        .sorted()
        .collect_vec();
    shuffle(&mut destructible_wall_positions, rng);
    for position in destructible_wall_positions
        .iter()
        .take(num_of_destructible_walls_to_place)
        .cloned()
    {
        commands
            .spawn((
                SpriteBundle {
                    texture: game_textures
                        .get_map_textures(world_type)
                        .destructible_wall
                        .clone(),
                    transform: Transform::from_xyz(
                        get_x(position.x),
                        get_y(position.y),
                        DESTRUCTIBLE_WALL_Z_LAYER,
                    ),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(TILE_WIDTH as f32, TILE_HEIGHT as f32)),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                Wall,
                Solid,
                Destructible,
                position,
            ))
            .add_rollback();
    }
}

pub fn setup_round(
    rng: &mut SessionRng,
    commands: &mut Commands,
    map_size: MapSize,
    world_type: WorldType,
    game_textures: &GameTextures,
    fonts: &Fonts,
    hud_colors: &HUDColors,
    number_of_players: u8,
    round_start_frame: u32,
) {
    let player_ids = (0..number_of_players)
        .map(PlayerID)
        .collect::<Vec<PlayerID>>();

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
                hud_colors,
                fonts,
                (map_size.columns as u32 * TILE_WIDTH) as f32,
                world_type,
                game_textures,
                &player_ids,
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
    let mut possible_player_spawn_positions = possible_player_spawn_positions
        .iter()
        .map(|(y, x)| Position { y: *y, x: *x });

    let mut player_spawn_positions = vec![];
    for player_id in player_ids {
        let player_spawn_position = possible_player_spawn_positions.next().unwrap();
        let base_texture = game_textures.get_player_texture(player_id).clone();
        commands
            .spawn((
                SpriteBundle {
                    texture: base_texture.clone(),
                    transform: Transform::from_xyz(
                        get_x(player_spawn_position.x),
                        get_y(player_spawn_position.y),
                        PLAYER_Z_LAYER,
                    ),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(TILE_WIDTH as f32, TILE_HEIGHT as f32)),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                Player {
                    id: player_id,
                    can_push_bombs: false,
                },
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
        rng,
        commands,
        game_textures,
        world_type,
        map_size,
        &player_spawn_positions,
    );

    commands.insert_resource(GameEndFrame(round_start_frame + ROUND_DURATION_SECS * FPS));
    commands.insert_resource(WallOfDeath::Dormant {
        activation_frame: round_start_frame + ROUND_DURATION_SECS / 2 * FPS,
    });
}

pub fn generate_item_at_position(
    rng: &mut SessionRng,
    commands: &mut Commands,
    game_textures: &GameTextures,
    position: Position,
) {
    let roll = rng.gen_u64() % 100;

    /* "Loot tables" */
    let item = match roll {
        _ if roll < 50 => Item::BombsUp,
        50..=89 => Item::RangeUp,
        _ if roll >= 90 => Item::BombPush,
        _ => unreachable!(),
    };

    commands
        .spawn((
            SpriteBundle {
                texture: match item {
                    Item::BombsUp => game_textures.bombs_up.clone(),
                    Item::RangeUp => game_textures.range_up.clone(),
                    Item::BombPush => game_textures.bomb_push.clone(),
                },
                transform: Transform::from_xyz(get_x(position.x), get_y(position.y), ITEM_Z_LAYER),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(TILE_WIDTH as f32, TILE_HEIGHT as f32)),
                    ..Default::default()
                },
                ..Default::default()
            },
            position,
            item,
        ))
        .add_rollback();
}

pub fn burn_item(
    commands: &mut Commands,
    game_textures: &GameTextures,
    item_entity: Entity,
    item_texture: &mut Handle<Image>,
    current_frame: u32,
) {
    commands
        .entity(item_entity)
        .remove::<Item>()
        .insert(BurningItem {
            expiration_frame: current_frame + FPS / 2,
        });
    *item_texture = game_textures.burning_item.clone();
}

pub fn setup_leaderboard_display(
    rng: &mut SessionRng,
    parent: &mut ChildBuilder,
    window_height: f32,
    window_width: f32,
    game_textures: &GameTextures,
    fonts: &Fonts,
    leaderboard: &Leaderboard,
    round_outcome: RoundOutcome,
) {
    parent
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    height: Val::Px(window_height),
                    width: Val::Px(window_width),
                    ..Default::default()
                },
                background_color: COLORS[0].into(),
                ..Default::default()
            },
            UIComponent,
            LeaderboardUIRoot,
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    NodeBundle {
                        style: Style {
                            position_type: PositionType::Absolute,
                            height: Val::Px(window_height),
                            width: Val::Px(window_width),
                            ..Default::default()
                        },
                        background_color: COLORS[0].into(),
                        ..Default::default()
                    },
                    UIComponent,
                    LeaderboardUIContent,
                ))
                .with_children(|parent| {
                    for (&player_id, &score) in &leaderboard.scores {
                        // spawn player portrait
                        parent
                            .spawn((
                                NodeBundle {
                                    style: Style {
                                        position_type: PositionType::Absolute,
                                        left: Val::Px(4.0 * PIXEL_SCALE as f32),
                                        top: Val::Px(
                                            ((6 + player_id.0 * 12) as u32 * PIXEL_SCALE) as f32,
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
                                            .get_player_texture(player_id)
                                            .clone()
                                            .into(),
                                        ..Default::default()
                                    },
                                    UIComponent,
                                ));
                            });

                        // spawn player trophies
                        for i in 0..score {
                            parent.spawn((
                                ImageBundle {
                                    style: Style {
                                        position_type: PositionType::Absolute,
                                        top: Val::Px(
                                            ((7 + player_id.0 * 12) as u32 * PIXEL_SCALE) as f32,
                                        ),
                                        left: Val::Px(((15 + i * 9) as u32 * PIXEL_SCALE) as f32),
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

                        if let RoundOutcome::Winner(round_winner_player_id) = round_outcome {
                            if player_id == round_winner_player_id {
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

                                place_text(6 + player_id.0 * 12, 15 + (score - 1) * 9 - 2, "*", 15);
                                place_text(8 + player_id.0 * 12, 15 + (score - 1) * 9 + 6, "*", 15);
                                place_text(
                                    10 + player_id.0 * 12,
                                    15 + (score - 1) * 9 - 1,
                                    "*",
                                    15,
                                );
                            }
                        }
                    }
                });

            // spawn border
            let mut spawn_color = |y: u32, x: u32| {
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
                            .nth(rng.gen_u64() as usize % COLORS.len())
                            .unwrap())
                        .into(),
                        ..Default::default()
                    },
                    UIComponent,
                ));
            };

            let height = window_height as u32 / PIXEL_SCALE;
            let width = window_width as u32 / PIXEL_SCALE;
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
        });
}

pub fn setup_tournament_winner_display(
    parent: &mut ChildBuilder,
    window_height: f32,
    window_width: f32,
    game_textures: &GameTextures,
    fonts: &Fonts,
    winner: PlayerID,
) {
    let center_y = window_height / 2.0 - (4 * PIXEL_SCALE) as f32 /* accounting for the chicken dinner text */;
    let center_x = window_width / 2.0;
    let portrait_trophy_distance = (6 * PIXEL_SCALE) as f32;

    // spawn the winning player portrait
    parent
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    top: Val::Px(center_y - TILE_HEIGHT as f32 / 2.0),
                    left: Val::Px(center_x - TILE_WIDTH as f32 - portrait_trophy_distance / 2.0),
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
                    image: game_textures.get_player_texture(winner).clone().into(),
                    ..Default::default()
                },
                UIComponent,
            ));
        });

    // spawn the winner trophy
    parent.spawn((
        ImageBundle {
            style: Style {
                position_type: PositionType::Absolute,
                top: Val::Px(center_y - (TILE_HEIGHT / 2 - PIXEL_SCALE) as f32),
                left: Val::Px(center_x + portrait_trophy_distance / 2.0),
                width: Val::Px(5.0 * PIXEL_SCALE as f32),
                height: Val::Px(7.0 * PIXEL_SCALE as f32),
                ..Default::default()
            },
            image: game_textures.trophy.clone().into(),
            ..Default::default()
        },
        UIComponent,
    ));

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
                    top: Val::Px(center_y + y as f32 * PIXEL_SCALE as f32),
                    left: Val::Px(center_x + x as f32 * PIXEL_SCALE as f32),
                    ..Default::default()
                },
                ..Default::default()
            },
            UIComponent,
        ));
    };

    // trophy sparkles
    place_text(-4, 1, "*", 15);
    place_text(-2, 9, "*", 15);
    place_text(0, 2, "*", 15);

    place_text(
        (TILE_WIDTH / PIXEL_SCALE / 2) as isize + 4,
        -14,
        "WINNER WINNER CHICKEN DINNER!",
        15,
    );
}
