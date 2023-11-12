use bevy::{
    prelude::{
        BuildChildren, ChildBuilder, Commands, Entity, NodeBundle, TextBundle, Transform, Vec2,
    },
    sprite::{Sprite, SpriteBundle},
    text::{Text, TextStyle},
    ui::{PositionType, Style, Val},
    utils::HashSet,
};
use bevy_ggrs::AddRollbackCommandExtension;
use itertools::Itertools;
use rand::{rngs::StdRng, seq::IteratorRandom, SeedableRng};

use crate::{
    components::{
        Destructible, GameTimerDisplay, HUDRoot, PenguinPortraitDisplay, Position, Solid,
        UIComponent, Wall,
    },
    constants::{COLORS, HUD_HEIGHT, PIXEL_SCALE, TILE_HEIGHT, TILE_WIDTH},
    resources::{Fonts, GameTextures, HUDColors, MapSize, WorldID},
    types::Direction,
};

pub fn get_x(x: isize) -> f32 {
    TILE_WIDTH as f32 / 2.0 + (x * TILE_WIDTH as isize) as f32
}

pub fn get_y(y: isize) -> f32 {
    -(TILE_HEIGHT as f32 / 2.0 + (y * TILE_HEIGHT as isize) as f32)
}

pub fn init_hud(
    parent: &mut ChildBuilder,
    hud_colors: &HUDColors,
    fonts: &Fonts,
    width: f32,
    world_id: WorldID,
    with_penguin_portrait_display: bool,
    with_clock: bool,
    extra_item_fn: Option<&dyn Fn(&mut ChildBuilder)>,
) {
    let mut ec = parent.spawn((
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
    ));
    ec.with_children(|parent| {
        if let Some(extra_item_fn) = extra_item_fn {
            extra_item_fn(parent);
        }

        if with_clock {
            // clock / pause indicator
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
        }
    });

    if with_penguin_portrait_display {
        ec.insert(PenguinPortraitDisplay);
    }
}

pub fn spawn_map(
    commands: &mut Commands,
    game_textures: &GameTextures,
    map_size: MapSize,
    percent_of_passable_positions_to_fill: f32,
    spawn_middle_blocks: bool,
    penguin_spawn_positions: &[Position],
) -> Vec<Vec<Entity>> {
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
    let mut stone_wall_spawn_groups = vec![];
    for i in 0..map_size.rows {
        let left = Position {
            y: i as isize,
            x: 0,
        };
        let right = Position {
            y: i as isize,
            x: (map_size.columns - 1) as isize,
        };
        stone_wall_spawn_groups.push(vec![left, right]);
    }
    for i in 1..map_size.columns - 1 {
        let top = Position {
            y: 0,
            x: i as isize,
        };
        let bottom = Position {
            y: (map_size.rows - 1) as isize,
            x: i as isize,
        };
        stone_wall_spawn_groups.push(vec![top, bottom]);
    }
    // checkered middle
    if spawn_middle_blocks {
        for i in (2..map_size.rows).step_by(2) {
            for j in (2..map_size.columns).step_by(2) {
                let position = Position {
                    y: i as isize,
                    x: j as isize,
                };
                stone_wall_spawn_groups.push(vec![position]);
            }
        }
    }

    let mut wall_entity_reveal_groups = vec![];
    for spawn_group in stone_wall_spawn_groups.iter() {
        let mut reveal_group = vec![];
        for position in spawn_group {
            let entity = commands
                .spawn((
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
                    *position,
                ))
                .id();
            reveal_group.push(entity);
        }
        wall_entity_reveal_groups.push(reveal_group);
    }

    let stone_wall_positions: HashSet<Position> =
        stone_wall_spawn_groups.iter().flatten().copied().collect();
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
    for position in &destructible_wall_positions {
        let entity = commands
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
                *position,
            ))
            .add_rollback()
            .id();
        wall_entity_reveal_groups.push(vec![entity]);
    }

    wall_entity_reveal_groups
}
