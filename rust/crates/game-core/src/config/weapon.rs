//! Port of `GameConfig/Models/WeaponConfig.cs`.

use serde::Deserialize;

/// Per-weapon tuning. All weapons share the base fields; type-specific fields
/// are `Option` and ignored when absent.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WeaponConfig {
    // Game-layer shot parameters.
    pub fire_rate: f32,
    pub projectile_speed: f32,
    pub time_to_live: f32,

    // WeaponProfile (fracture engine).
    pub directionality: f32,
    pub blast_fraction: f32,
    pub knockback: f32,

    // Pellet variance + drag (shotgun / shrapnel).
    pub speed_jitter: f32,
    pub ttl_jitter: f32,
    pub spread_jitter: f32,
    pub drag: f32,

    // Piercing physical mass.
    pub mass: Option<f32>,

    // Shotgun.
    pub rays: Option<i32>,
    pub cone_angle: Option<f32>,

    // Grenade.
    pub fuse_time: Option<f32>,
    pub shrapnel_count: Option<i32>,
    pub shrapnel_spread: Option<f32>,
    pub shrapnel_speed: Option<f32>,
    pub shrapnel_mass: Option<f32>,

    // Piercing.
    pub lateral_impulse_clamp: Option<f32>,
    /// Superseded by the penetration-power model. Kept so old configs still load.
    pub penetration_speed_loss: Option<f32>,
    pub penetration_power: Option<f32>,
    pub penetration_cost_scale: Option<f32>,
    pub pierce_damage_scale: Option<f32>,
    pub pierce_speed_exponent: Option<f32>,
    pub target_push_coeff: Option<f32>,

    pub shape_scale: Option<f32>,
}

impl Default for WeaponConfig {
    fn default() -> Self {
        Self {
            fire_rate: 4.0,
            projectile_speed: 900.0,
            time_to_live: 2.5,
            directionality: 0.4,
            blast_fraction: 0.3,
            knockback: 0.01,
            speed_jitter: 0.0,
            ttl_jitter: 0.0,
            spread_jitter: 0.0,
            drag: 0.0,
            mass: None,
            rays: None,
            cone_angle: None,
            fuse_time: None,
            shrapnel_count: None,
            shrapnel_spread: None,
            shrapnel_speed: None,
            shrapnel_mass: None,
            lateral_impulse_clamp: None,
            penetration_speed_loss: None,
            penetration_power: None,
            penetration_cost_scale: None,
            pierce_damage_scale: None,
            pierce_speed_exponent: None,
            target_push_coeff: None,
            shape_scale: None,
        }
    }
}
