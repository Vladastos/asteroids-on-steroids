//! Port of `GameConfig/Models/EntityConfig.cs`.

use serde::Deserialize;

/// Alien ship prefab config. Shape file is loaded from `Assets/shapes/`.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EntityConfig {
    pub shape: String,
    /// Optional material override; empty = use the shape's own material.
    pub material: String,
    pub speed: f32,
    pub detection_radius: f32,
    pub steering_weights: Option<SteeringWeights>,
    pub thrust: f32,
    /// Max aim turn rate (rad/s).
    pub turn_speed: f32,
    pub shoot_cooldown: f32,
    pub lateral_thrust_penalty_mult: f32,
    pub alien_impact_coeff: f32,
    pub shape_scale: f32,
    pub base_cost: f32,
    pub cell_count: i32,
    pub boss: Option<BossConfig>,
    /// Beyond this radius the alien ignores the player and wanders. 0 = always aggro.
    pub aggro_radius: f32,
    /// Standoff distance a kiting alien tries to hold. 0 = pursue directly.
    pub preferred_range: f32,
    pub dash: Option<AlienDashConfig>,
}

impl Default for EntityConfig {
    fn default() -> Self {
        Self {
            shape: String::new(),
            material: String::new(),
            speed: 200.0,
            detection_radius: 800.0,
            steering_weights: None,
            thrust: 600.0,
            turn_speed: 7.0,
            shoot_cooldown: 2.0,
            lateral_thrust_penalty_mult: 0.4,
            alien_impact_coeff: 1.0,
            shape_scale: 1.0,
            base_cost: 20.0,
            cell_count: 8,
            boss: None,
            aggro_radius: 0.0,
            preferred_range: 0.0,
            dash: None,
        }
    }
}

/// A cooldown-gated lunge toward the player, triggered when within `trigger_range`.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AlienDashConfig {
    pub cooldown: f32,
    pub trigger_range: f32,
    pub speed: f32,
}

impl Default for AlienDashConfig {
    fn default() -> Self {
        Self {
            cooldown: 8.0,
            trigger_range: 400.0,
            speed: 1500.0,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BossConfig {
    pub shockwave_cooldown: f32,
    pub shockwave_radius: f32,
    pub shockwave_strength: f32,
    pub black_hole_cooldown: f32,
    pub black_hole_radius: f32,
    pub black_hole_strength: f32,
    pub black_hole_crush_radius: f32,
    pub black_hole_duration: f32,
    pub black_hole_speed: f32,
    pub ram_charge_cooldown: f32,
    pub ram_charge_min_dist: f32,
    pub ram_charge_duration: f32,
    /// Homing lunge speed (mass-independent).
    pub ram_charge_speed: f32,
    /// How fast it reaches ram speed (px/s²).
    pub ram_charge_accel: f32,
    pub spawn_interval: f32,
    pub spawn_type: String,
    pub spawn_safety_margin: f32,
    pub drift_thrust: f32,

    // Movement: velocity-model standoff pursuit.
    pub cruise_speed: f32,
    pub preferred_range: f32,
    pub accel: f32,

    // Overdrive + first-cast delays.
    pub overdrive_cockpit_fraction: f32,
    pub overdrive_spawn_mult: f32,
    pub black_hole_initial_delay: f32,
    pub ram_charge_initial_delay: f32,

    // Black hole targeting.
    /// 0 = aim at current pos, 1 = full intercept lead.
    pub black_hole_lead: f32,

    // Radial bullet barrage.
    pub barrage_cooldown: f32,
    pub barrage_count: i32,
    pub barrage_speed: f32,
    pub barrage_initial_delay: f32,
    pub barrage_overdrive_mult: f32,
    pub barrage_speed_jitter: f32,
    pub barrage_spread_jitter: f32,
    pub barrage_ttl_jitter: f32,
    pub barrage_spawn_radius: f32,
    /// Impact mass of each barrage ray. `None` = fall back to the cannon_alien weapon mass.
    pub barrage_ray_mass: Option<f32>,
}

impl Default for BossConfig {
    fn default() -> Self {
        Self {
            shockwave_cooldown: 8.0,
            shockwave_radius: 1200.0,
            shockwave_strength: 80000.0,
            black_hole_cooldown: 15.0,
            black_hole_radius: 500.0,
            black_hole_strength: 50000.0,
            black_hole_crush_radius: 40.0,
            black_hole_duration: 6.0,
            black_hole_speed: 200.0,
            ram_charge_cooldown: 12.0,
            ram_charge_min_dist: 600.0,
            ram_charge_duration: 2.5,
            ram_charge_speed: 850.0,
            ram_charge_accel: 1400.0,
            spawn_interval: 8.0,
            spawn_type: "drone".into(),
            spawn_safety_margin: 80.0,
            drift_thrust: 150.0,
            cruise_speed: 90.0,
            preferred_range: 750.0,
            accel: 220.0,
            overdrive_cockpit_fraction: 0.5,
            overdrive_spawn_mult: 0.5,
            black_hole_initial_delay: 0.4,
            ram_charge_initial_delay: 0.7,
            black_hole_lead: 1.0,
            barrage_cooldown: 6.0,
            barrage_count: 24,
            barrage_speed: 650.0,
            barrage_initial_delay: 0.6,
            barrage_overdrive_mult: 0.6,
            barrage_speed_jitter: 0.25,
            barrage_spread_jitter: 0.6,
            barrage_ttl_jitter: 0.3,
            barrage_spawn_radius: 220.0,
            barrage_ray_mass: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SteeringWeights {
    pub separation: f32,
    pub pursuit: f32,
    pub avoidance: f32,
}

impl Default for SteeringWeights {
    fn default() -> Self {
        Self {
            separation: 1.0,
            pursuit: 1.0,
            avoidance: 1.0,
        }
    }
}
