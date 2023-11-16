use bevy::{
    prelude::{BuildChildren, ChildBuilder, Commands, NodeBundle, TextBundle, Transform, Vec2},
    sprite::{Sprite, SpriteBundle},
    text::{Text, TextStyle},
    ui::{node_bundles::ImageBundle, PositionType, Style, UiRect, Val},
    utils::HashSet,
};
use bevy_ggrs::AddRollbackCommandExtension;
use itertools::Itertools;
use rand::{rngs::StdRng, seq::IteratorRandom, SeedableRng};

use crate::{
    components::{
        Destructible, GameTimerDisplay, HUDRoot, Item, Penguin, PenguinPortrait,
        PenguinPortraitDisplay, Position, Solid, UIComponent, Wall,
    },
    constants::{
        BATTLE_MODE_ROUND_DURATION_SECS, COLORS, HUD_HEIGHT, PIXEL_SCALE, TILE_HEIGHT, TILE_WIDTH,
    },
    resources::{Fonts, GameTextures, HUDColors, MapSize, WorldID},
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

pub fn init_hud(
    parent: &mut ChildBuilder,
    hud_colors: &HUDColors,
    fonts: &Fonts,
    width: f32,
    world_id: WorldID,
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
                background_color: hud_colors.get_background_color(world_id).into(),
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
                                format_hud_time(BATTLE_MODE_ROUND_DURATION_SECS),
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

pub fn spawn_map(
    commands: &mut Commands,
    game_textures: &GameTextures,
    map_size: MapSize,
    percent_of_passable_positions_to_fill: f32,
    spawn_middle_blocks: bool,
    penguin_spawn_positions: &[Position],
) {
    // TODO make truly random
    let mut rng = StdRng::seed_from_u64(42);

    // place empty/passable tiles
    for j in 0..map_size.rows {
        for i in 0..map_size.columns {
            commands.spawn(SpriteBundle {
                texture: game_textures.get_map_textures().empty.clone(),
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
    if spawn_middle_blocks {
        for i in (2..map_size.rows).step_by(2) {
            for j in (2..map_size.columns).step_by(2) {
                stone_wall_positions.insert(Position {
                    y: i as isize,
                    x: j as isize,
                });
            }
        }
    }

    for position in stone_wall_positions.iter().cloned() {
        commands.spawn((
            SpriteBundle {
                texture: game_textures.get_map_textures().wall.clone(),
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

    // reserve room for the penguins (cross-shaped)
    for penguin_spawn_position in penguin_spawn_positions {
        destructible_wall_potential_positions.remove(penguin_spawn_position);
        for position in Direction::LIST
            .iter()
            .map(|direction| penguin_spawn_position.offset(*direction, 1))
        {
            destructible_wall_potential_positions.remove(&position);
        }
    }

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
        .choose_multiple(&mut rng, num_of_destructible_walls_to_place);
    for position in destructible_wall_positions.iter().cloned() {
        commands
            .spawn((
                SpriteBundle {
                    texture: game_textures.get_map_textures().destructible_wall.clone(),
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

pub fn generate_item_at_position(
    position: Position,
    commands: &mut Commands,
    game_textures: &GameTextures,
) {
    // TODO
    // let r = rand::thread_rng().gen::<usize>() % 100;
    let r = 42;

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
