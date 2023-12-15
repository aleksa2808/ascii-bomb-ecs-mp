use bevy::{ecs as bevy_ecs, prelude::*, text::Font, utils::HashMap};
use bevy_matchbox::matchbox_socket::PeerId;
use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoshiro256StarStar;

use crate::{
    components::Position,
    constants::COLORS,
    types::{Cooldown, Direction, ICEServerConfig, PlayerID, PostFreezeAction},
};

#[derive(Resource)]
pub struct NetworkStatsCooldown {
    pub cooldown: Cooldown,
    pub print_cooldown: u32,
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
    background_colors: HashMap<WorldType, Color>,
    pub black_color: Color,
    pub portrait_background_color: Color,
    pub portrait_border_color: Color,
}

impl HUDColors {
    pub fn get_background_color(&self, world_type: WorldType) -> Color {
        self.background_colors[&world_type]
    }
}

impl Default for HUDColors {
    fn default() -> Self {
        let background_colors: HashMap<WorldType, Color> = [
            (WorldType::GrassWorld, Color::into(COLORS[2].into())),
            (WorldType::IceWorld, Color::into(COLORS[11].into())),
            (WorldType::CloudWorld, Color::into(COLORS[3].into())),
        ]
        .into_iter()
        .collect();
        assert!(background_colors.len() == WorldType::LIST.len());

        Self {
            background_colors,
            black_color: COLORS[0].into(),
            portrait_background_color: COLORS[3].into(),
            portrait_border_color: COLORS[8].into(),
        }
    }
}

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
    map_textures: HashMap<WorldType, MapTextures>,
    pub bombs_up: Handle<Image>,
    pub range_up: Handle<Image>,
    pub bomb_push: Handle<Image>,
    pub burning_item: Handle<Image>,
    pub trophy: Handle<Image>,
}

impl GameTextures {
    pub fn get_map_textures(&self, world_type: WorldType) -> &MapTextures {
        &self.map_textures[&world_type]
    }

    pub fn get_player_texture(&self, player_id: PlayerID) -> &Handle<Image> {
        self.penguin_variants
            .iter()
            .cycle()
            .nth(player_id.0 as usize)
            .unwrap()
    }
}

impl FromWorld for GameTextures {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.get_resource::<AssetServer>().unwrap();

        let map_textures: HashMap<WorldType, MapTextures> = WorldType::LIST
            .into_iter()
            .enumerate()
            .map(|(i, world_type)| {
                let world_id = i + 1;
                (
                    world_type,
                    MapTextures {
                        empty: asset_server.load(format!("sprites/world/{}/empty.png", world_id)),
                        wall: asset_server.load(format!("sprites/world/{}/wall.png", world_id)),
                        destructible_wall: asset_server
                            .load(format!("sprites/world/{}/destructible_wall.png", world_id)),
                        burning_wall: asset_server
                            .load(format!("sprites/world/{}/burning_wall.png", world_id)),
                    },
                )
            })
            .collect();

        let penguin_variants: Vec<Handle<Image>> = (0..=14)
            .map(|i| asset_server.load(format!("sprites/penguins/{}.png", i)))
            .collect();

        let bomb_texture = asset_server.load("sprites/bomb.png");
        let fire_texture = asset_server.load("sprites/fire.png");
        let bombs_up_texture = asset_server.load("sprites/bombs_up.png");
        let range_up_texture = asset_server.load("sprites/range_up.png");
        let bomb_push_texture = asset_server.load("sprites/bomb_push.png");
        let burning_item_texture = asset_server.load("sprites/burning_item.png");
        let trophy_texture = asset_server.load("sprites/trophy.png");

        GameTextures {
            penguin_variants: penguin_variants.to_vec(),
            bomb: bomb_texture.clone(),
            fire: fire_texture.clone(),
            map_textures,
            bombs_up: bombs_up_texture.clone(),
            range_up: range_up_texture.clone(),
            bomb_push: bomb_push_texture.clone(),
            burning_item: burning_item_texture.clone(),
            trophy: trophy_texture.clone(),
        }
    }
}

#[derive(Resource, Clone, Copy)]
pub struct MapSize {
    pub rows: u8,
    pub columns: u8,
}

#[derive(Resource, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(clippy::enum_variant_names)]
pub enum WorldType {
    GrassWorld,
    IceWorld,
    CloudWorld,
}

impl WorldType {
    pub const LIST: [Self; 3] = [Self::GrassWorld, Self::IceWorld, Self::CloudWorld];

    pub fn random(rng: &mut SessionRng) -> Self {
        match rng.gen_u64() % 3 {
            0 => Self::GrassWorld,
            1 => Self::IceWorld,
            2 => Self::CloudWorld,
            _ => unreachable!(),
        }
    }

    pub fn next_random(&self, rng: &mut SessionRng) -> Self {
        Self::LIST
            .into_iter()
            .filter(|&w| w != *self)
            .nth((rng.gen_u64() as usize) % (Self::LIST.len() - 1))
            .unwrap()
    }
}

#[derive(Resource)]
pub struct MatchboxConfig {
    pub number_of_players: u8,
    pub room_id: String,
    pub matchbox_server_url: Option<String>,
    pub ice_server_config: Option<ICEServerConfig>,
}

#[derive(Resource)]
pub struct RngSeeds {
    pub local: u64,
    pub remote: HashMap<PeerId, Option<u64>>,
}

// I could not verify it but I assume that the Xoshiro256StarStar generator is platform-independent. This is necessary for cross-platform deterministic gameplay.
#[derive(Resource, Clone)]
pub struct SessionRng(Xoshiro256StarStar);

impl SessionRng {
    pub fn new(seed: u64) -> Self {
        Self(Xoshiro256StarStar::seed_from_u64(seed))
    }

    // Allow only `u64` number generation in order to prevent things like generating platform dependent `usize` values.
    pub fn gen_u64(&mut self) -> u64 {
        self.0.gen()
    }
}

#[derive(Resource)]
pub struct LocalPlayerID(pub u8);

#[derive(Resource)]
pub struct Leaderboard {
    pub scores: HashMap<PlayerID, u8>,
    pub winning_score: u8,
}

#[derive(Resource, Clone, Copy)]
pub struct FrameCount {
    pub frame: u32,
}

#[derive(Resource, Clone, Copy)]
pub enum WallOfDeath {
    Dormant {
        activation_frame: u32,
    },
    Active {
        position: Position,
        direction: Direction,
        next_step_frame: u32,
    },
    Done,
}

#[derive(Resource)]
pub struct GameEndFrame(pub u32);

#[derive(Resource, Clone, Copy)]
pub struct GameFreeze {
    pub end_frame: u32,
    pub post_freeze_action: Option<PostFreezeAction>,
}
