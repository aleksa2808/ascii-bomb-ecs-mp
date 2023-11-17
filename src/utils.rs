use bevy::{
    prelude::{BuildChildren, ChildBuilder, Commands, NodeBundle, TextBundle, Transform, Vec2},
    render::color::Color,
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
        BombSatchel, Destructible, GameTimerDisplay, HUDRoot, Item, Penguin, PenguinPortrait,
        PenguinPortraitDisplay, Player, Position, Solid, UIComponent, UIRoot, Wall,
    },
    constants::{
        BATTLE_MODE_ROUND_DURATION_SECS, COLORS, FPS, HUD_HEIGHT, PIXEL_SCALE, TILE_HEIGHT,
        TILE_WIDTH,
    },
    resources::{Fonts, GameEndFrame, GameTextures, HUDColors, MapSize, WorldType},
    types::Direction,
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
    penguin_tags: &[Penguin],
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
            PenguinPortraitDisplay,
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
                                "",
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
            for penguin in penguin_tags {
                parent
                    .spawn((
                        NodeBundle {
                            style: Style {
                                position_type: PositionType::Absolute,
                                left: Val::Px(((5 + 12 * penguin.0) * PIXEL_SCALE) as f32),
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
                        PenguinPortrait(*penguin),
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
                                            .get_penguin_texture(*penguin)
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
                transform: Transform::from_xyz(get_x(position.x), get_y(position.y), 10.0),
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
                    transform: Transform::from_xyz(get_x(position.x), get_y(position.y), 10.0),
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
    mut commands: Commands,
    map_size: MapSize,
    world_type: WorldType,
    game_textures: &GameTextures,
    fonts: &Fonts,
    hud_colors: &HUDColors,
    number_of_players: usize,
    current_frame: usize,
) {
    let penguin_tags = (0..number_of_players)
        .map(Penguin)
        .collect::<Vec<Penguin>>();

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
                world_type,
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
        rng,
        &mut commands,
        &game_textures,
        world_type,
        map_size,
        &player_spawn_positions,
    );

    commands.insert_resource(GameEndFrame(
        current_frame + BATTLE_MODE_ROUND_DURATION_SECS * FPS,
    ));
}

pub fn generate_item_at_position(
    rng: &mut StdRng,
    position: Position,
    commands: &mut Commands,
    game_textures: &GameTextures,
) {
    let r = rng.gen::<usize>() % 100;

    /* "Loot tables" */
    let item = match r {
        _ if r < 50 => Item::BombsUp,
        50..=89 => Item::RangeUp,
        _ if r >= 90 => Item::BombPush,
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
                transform: Transform::from_xyz(get_x(position.x), get_y(position.y), 20.0),
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
