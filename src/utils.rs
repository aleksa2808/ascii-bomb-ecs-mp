use bevy::{
    asset::Handle,
    ecs::entity::Entity,
    prelude::{BuildChildren, ChildBuilder, Commands, NodeBundle, TextBundle, Transform, Vec2},
    render::{color::Color, texture::Image},
    sprite::{Sprite, SpriteBundle},
    text::{Text, TextStyle},
    ui::{node_bundles::ImageBundle, PositionType, Style, UiRect, Val},
    utils::HashSet,
};
use bevy_ggrs::AddRollbackCommandExtension;
use itertools::Itertools;
use rand::{rngs::StdRng, seq::IteratorRandom, Rng};

use crate::{
    components::{
        BombSatchel, BurningItem, Destructible, GameTimerDisplay, HUDRoot, Item, LeaderboardUI,
        Player, PlayerPortrait, PlayerPortraitDisplay, Position, Solid, UIComponent, UIRoot, Wall,
    },
    constants::{
        COLORS, DESTRUCTIBLE_WALL_Z_LAYER, FPS, HUD_HEIGHT, ITEM_Z_LAYER, PIXEL_SCALE,
        PLAYER_Z_LAYER, ROUND_DURATION_SECS, TILE_HEIGHT, TILE_WIDTH, WALL_Z_LAYER,
    },
    resources::{
        Fonts, GameEndFrame, GameTextures, HUDColors, Leaderboard, MapSize, WallOfDeath, WorldType,
    },
    types::{Direction, PlayerID, RoundOutcome},
};

pub fn get_x(x: isize) -> f32 {
    TILE_WIDTH as f32 / 2.0 + (x * TILE_WIDTH as isize) as f32
}

pub fn get_y(y: isize) -> f32 {
    -(TILE_HEIGHT as f32 / 2.0 + (y * TILE_HEIGHT as isize) as f32)
}

pub fn format_hud_time(remaining_seconds: usize) -> String {
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

            // player portraits
            for &player_id in player_ids {
                parent
                    .spawn((
                        NodeBundle {
                            style: Style {
                                position_type: PositionType::Absolute,
                                left: Val::Px(((5 + 12 * player_id.0) * PIXEL_SCALE) as f32),
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
    rng: &mut StdRng,
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
                transform: Transform::from_xyz(get_x(i as isize), get_y(j as isize), 0.0),
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
        stone_wall_positions.insert(Position {
            y: i as isize,
            x: 0,
        });
        // right
        stone_wall_positions.insert(Position {
            y: i as isize,
            x: (map_size.columns - 1) as isize,
        });
    }
    for i in 1..map_size.columns - 1 {
        // top
        stone_wall_positions.insert(Position {
            y: 0,
            x: i as isize,
        });
        // bottom
        stone_wall_positions.insert(Position {
            y: (map_size.rows - 1) as isize,
            x: i as isize,
        });
    }
    // checkered middle
    for i in (2..map_size.rows).step_by(2) {
        for j in (2..map_size.columns).step_by(2) {
            stone_wall_positions.insert(Position {
                y: i as isize,
                x: j as isize,
            });
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
        .flat_map(|y| {
            (0..map_size.columns).map(move |x| Position {
                y: y as isize,
                x: x as isize,
            })
        })
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
    // TODO remove f32 and this panic
    let percent_of_passable_positions_to_fill = if number_of_players > 4 { 70.0 } else { 60.0 };
    let num_of_destructible_walls_to_place = (number_of_passable_positions as f32
        * percent_of_passable_positions_to_fill
        / 100.0) as usize;
    if destructible_wall_potential_positions.len() < num_of_destructible_walls_to_place {
        panic!(
            "Not enough passable positions available for placing destructible walls. Have {}, but need at least {}",
            destructible_wall_potential_positions.len(),
            num_of_destructible_walls_to_place
        );
    }

    let destructible_wall_positions = destructible_wall_potential_positions
        .into_iter()
        .sorted_by_key(|p| (p.x, p.y))
        .choose_multiple(rng, num_of_destructible_walls_to_place);
    for position in destructible_wall_positions.iter().cloned() {
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
    rng: &mut StdRng,
    commands: &mut Commands,
    map_size: MapSize,
    world_type: WorldType,
    game_textures: &GameTextures,
    fonts: &Fonts,
    hud_colors: &HUDColors,
    number_of_players: usize,
    round_start_frame: usize,
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
                (map_size.columns * TILE_WIDTH) as f32,
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
    let mut possible_player_spawn_positions =
        possible_player_spawn_positions
            .iter()
            .map(|(y, x)| Position {
                y: *y as isize,
                x: *x as isize,
            });

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
    rng: &mut StdRng,
    commands: &mut Commands,
    game_textures: &GameTextures,
    position: Position,
) {
    let roll = rng.gen::<usize>() % 100;

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
    current_frame: usize,
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
    rng: &mut StdRng,
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
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    height: Val::Px(window_height),
                    width: Val::Px(window_width),
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
                        background_color: (*COLORS.iter().choose(rng).unwrap()).into(),
                        ..Default::default()
                    },
                    UIComponent,
                ));
            };

            let height = window_height as usize / PIXEL_SCALE;
            let width = window_width as usize / PIXEL_SCALE;
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

            for (&player_id, &score) in &leaderboard.scores {
                // spawn player portrait
                parent
                    .spawn((
                        NodeBundle {
                            style: Style {
                                position_type: PositionType::Absolute,
                                left: Val::Px(4.0 * PIXEL_SCALE as f32),
                                top: Val::Px(((6 + player_id.0 * 12) * PIXEL_SCALE) as f32),
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
                                image: game_textures.get_player_texture(player_id).clone().into(),
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
                                top: Val::Px(((7 + player_id.0 * 12) * PIXEL_SCALE) as f32),
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
                        place_text(10 + player_id.0 * 12, 15 + (score - 1) * 9 - 1, "*", 15);
                    }
                }
            }
        });
}
