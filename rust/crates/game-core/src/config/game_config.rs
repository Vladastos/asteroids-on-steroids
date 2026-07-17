//! Port of `GameConfig/Models/GameConfig.cs`.

use super::asteroid::AsteroidConfig;
use super::entity::EntityConfig;
use super::fracture_global::FractureGlobalConfig;
use super::material::MaterialConfig;
use super::physics::PhysicsConfig;
use super::player::PlayerConfig;
use super::skill::SkillConfig;
use super::vfx::VfxConfig;
use super::wave::WaveDefinition;
use super::wave_system::{
    BorderHazardConfig, VortexConfig, VortexFxConfig, WaveSystemConfig, WorldConfig,
};
use super::weapon::WeaponConfig;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GameConfig {
    pub materials: HashMap<String, MaterialConfig>,
    pub weapons: HashMap<String, WeaponConfig>,
    pub skills: HashMap<String, SkillConfig>,
    pub player: PlayerConfig,
    pub entities: HashMap<String, EntityConfig>,
    pub asteroids: HashMap<String, AsteroidConfig>,
    pub max_live_cells: i32,
    pub waves: Vec<WaveDefinition>,
    pub scoring: ScoringConfig,
    pub world: WorldConfig,
    pub border_hazard: BorderHazardConfig,
    pub vortex: VortexConfig,
    pub vortex_fx: VortexFxConfig,
    pub wave_system: WaveSystemConfig,
    pub vfx: VfxConfig,
    pub fracture: FractureGlobalConfig,
    pub physics: PhysicsConfig,
    pub difficulties: Vec<DifficultyConfig>,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            materials: HashMap::new(),
            weapons: HashMap::new(),
            skills: HashMap::new(),
            player: PlayerConfig::default(),
            entities: HashMap::new(),
            asteroids: HashMap::new(),
            max_live_cells: 600,
            waves: Vec::new(),
            scoring: ScoringConfig::default(),
            world: WorldConfig::default(),
            border_hazard: BorderHazardConfig::default(),
            vortex: VortexConfig::default(),
            vortex_fx: VortexFxConfig::default(),
            wave_system: WaveSystemConfig::default(),
            vfx: VfxConfig::default(),
            fracture: FractureGlobalConfig::default(),
            physics: PhysicsConfig::default(),
            difficulties: Vec::new(),
        }
    }
}

/// A named difficulty preset: a set of multipliers layered over the base
/// tuning at run start.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DifficultyConfig {
    pub name: String,
    /// Scales the per-wave spawn budget.
    pub budget_mult: f32,
    /// Scales the live-cell cap growth.
    pub cap_mult: f32,
    /// Scales alien fire cooldown (below 1 = faster fire = harder).
    pub enemy_fire_mult: f32,
    /// Scales damage the player takes (via `player_impact_coeff`).
    pub player_damage_mult: f32,
}

impl Default for DifficultyConfig {
    fn default() -> Self {
        Self {
            name: "Normal".into(),
            budget_mult: 1.0,
            cap_mult: 1.0,
            enemy_fire_mult: 1.0,
            player_damage_mult: 1.0,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScoringConfig {
    /// Score per cell = area × toughness × this weight.
    pub cell_score_area_weight: f32,
    /// Kill-chain multiplier tiers. Index 0 = first kill (×1).
    pub kill_chain_steps: Vec<f32>,
    /// Seconds without a kill before the chain resets to tier 0.
    pub kill_chain_decay: f32,
    pub wave_survival_bonus: i32,
    pub leaderboard_size: i32,
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            cell_score_area_weight: 0.001,
            kill_chain_steps: vec![1.0, 1.5, 2.0, 4.0],
            kill_chain_decay: 3.0,
            wave_survival_bonus: 500,
            leaderboard_size: 10,
        }
    }
}
