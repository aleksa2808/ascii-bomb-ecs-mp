use bevy::{ecs as bevy_ecs, prelude::*, reflect as bevy_reflect, text::Font, utils::HashMap};

use crate::{components::Penguin, constants::COLORS};

#[derive(Resource)]
pub struct MatchboxConfig {
    pub signal_server_address: String,
    pub room: Option<String>,
    pub number_of_players: usize,
}

#[derive(Resource)]
pub struct Fonts {
    pub mono: Handle<Font>,
}

impl FromWorld for Fonts {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.get_resource::<AssetServer>().unwrap();

        Fonts {
            mono: asset_server.load("fonts/UbuntuMono-R.ttf"),
        }
    }
}

#[derive(Resource)]
pub struct HUDColors {
    background_colors: Vec<Color>,
    pub black_color: Color,
    pub portrait_background_color: Color,
    pub portrait_border_color: Color,
}

impl HUDColors {
    pub fn get_background_color(&self, world_id: WorldID) -> Color {
        self.background_colors[world_id.0 - 1]
    }
}

impl Default for HUDColors {
    fn default() -> Self {
        Self {
            background_colors: vec![
                Color::into(COLORS[2].into()),
                Color::into(COLORS[11].into()),
                Color::into(COLORS[3].into()),
            ],
            black_color: COLORS[0].into(),
            portrait_background_color: COLORS[3].into(),
            portrait_border_color: COLORS[8].into(),
        }
    }
}

#[derive(Default)]
pub struct MapTextures {
    pub empty: Handle<Image>,
    pub wall: Handle<Image>,
    pub destructible_wall: Handle<Image>,
    pub burning_wall: Handle<Image>,
}

#[derive(Resource)]
pub struct GameTextures {
    penguin_variants: Vec<Handle<Image>>,
    pub bomb: Handle<Image>,
    pub fire: Handle<Image>,
    map_textures: Vec<MapTextures>,
    map_textures_index: usize,
    pub bombs_up: Handle<Image>,
    pub range_up: Handle<Image>,
    pub bomb_push: Handle<Image>,
    pub burning_item: Handle<Image>,
    pub trophy: Handle<Image>,
}

impl GameTextures {
    pub fn set_map_textures(&mut self, world_id: WorldID) {
        self.map_textures_index = world_id.0 - 1;
    }

    pub fn get_map_textures(&self) -> &MapTextures {
        &self.map_textures[self.map_textures_index]
    }

    pub fn get_penguin_texture(&self, penguin: Penguin) -> &Handle<Image> {
        self.penguin_variants.iter().cycle().nth(penguin.0).unwrap()
    }
}

impl FromWorld for GameTextures {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.get_resource::<AssetServer>().unwrap();

        let map_textures: Vec<MapTextures> = (1..=3)
            .map(|world_id| MapTextures {
                empty: asset_server.load(format!("sprites/world/{}/empty.png", world_id).as_str()),
                wall: asset_server.load(format!("sprites/world/{}/wall.png", world_id).as_str()),
                destructible_wall: asset_server
                    .load(format!("sprites/world/{}/destructible_wall.png", world_id).as_str()),
                burning_wall: asset_server
                    .load(format!("sprites/world/{}/burning_wall.png", world_id).as_str()),
            })
            .collect();

        let penguin_variants: Vec<Handle<Image>> = (0..=14)
            .map(|i| asset_server.load(format!("sprites/penguins/{}.png", i).as_str()))
            .collect();

        let bomb_texture = asset_server.load("sprites/bomb.png");
        let fire_texture = asset_server.load("sprites/fire.png");
        let bombs_up_texture = asset_server.load("sprites/bombs_up.png");
        let range_up_texture = asset_server.load("sprites/range_up.png");
        let bomb_push_texture = asset_server.load("sprites/bomb_push.png");
        let burning_item_texture = asset_server.load("sprites/burning_item.png");
        let trophy_texture = asset_server.load("sprites/trophy.png");

        let game_textures = GameTextures {
            penguin_variants: penguin_variants.to_vec(),
            bomb: bomb_texture.clone(),
            fire: fire_texture.clone(),
            map_textures: map_textures
                .iter()
                .map(|mt| MapTextures {
                    empty: mt.empty.clone(),
                    wall: mt.wall.clone(),
                    destructible_wall: mt.destructible_wall.clone(),
                    burning_wall: mt.burning_wall.clone(),
                })
                .collect(),
            map_textures_index: 0, // defaults to world 1
            bombs_up: bombs_up_texture.clone(),
            range_up: range_up_texture.clone(),
            bomb_push: bomb_push_texture.clone(),
            burning_item: burning_item_texture.clone(),
            trophy: trophy_texture.clone(),
        };

        game_textures
    }
}

#[derive(Clone, Copy, Resource)]
pub struct MapSize {
    pub rows: usize,
    pub columns: usize,
}

#[derive(Resource, Default, Clone, Copy)]
pub struct WorldID(pub usize);

#[derive(Resource, Default, Reflect, Hash)]
#[reflect(Hash)]
pub struct FrameCount {
    pub frame: usize,
}

#[derive(Resource, Reflect, Default)]
pub struct Leaderboard {
    pub scores: HashMap<Penguin, usize>,
    pub winning_score: usize,
}

#[derive(Resource, Reflect, Default, Hash)]
pub enum RoundOutcome {
    #[default]
    Tie,
    Winner(Penguin),
}

// #[derive(Resource)]
// pub struct GameEndFrame(pub usize);

#[derive(Resource, Reflect, Default, Hash)]
pub struct FreezeEndFrame(pub usize);

#[derive(Resource, Reflect, Default)]
pub struct TournamentComplete;
