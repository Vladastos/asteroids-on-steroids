//! Config models — port of `GameConfig/Models/*.cs`. All `serde::Deserialize`
//! with `#[serde(default)]` mirroring the C#'s per-field `= value` defaults, so
//! a config file only needs to specify what it overrides — same contract as
//! `System.Text.Json` with `JsonNamingPolicy.CamelCase`.

mod asteroid;
mod entity;
mod fracture_global;
mod game_config;
mod material;
mod physics;
mod player;
mod shape_data;
mod skill;
mod vfx;
mod wave;
mod wave_system;
mod weapon;

pub use asteroid::{AsteroidConfig, ProceduralAsteroidConfig, VortexResponseConfig};
pub use entity::{AlienDashConfig, BossConfig, EntityConfig, SteeringWeights};
pub use fracture_global::FractureGlobalConfig;
pub use game_config::{DifficultyConfig, GameConfig, ScoringConfig};
pub use material::MaterialConfig;
pub use physics::PhysicsConfig;
pub use player::PlayerConfig;
pub use shape_data::{RoleTag, SeedData, ShapeData};
pub use skill::SkillConfig;
pub use vfx::VfxConfig;
pub use wave::{ExplicitSpawn, WaveDefinition};
pub use wave_system::{
    BorderHazardConfig, CampingResponseConfig, SpawnBiasEntry, SpawnPatternConfig,
    SpecialWaveConfig, VortexConfig, VortexFxConfig, WaveSystemConfig, WorldConfig,
};
pub use weapon::WeaponConfig;
