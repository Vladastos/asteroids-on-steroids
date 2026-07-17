//! Port of `GameConfig/Models/FractureGlobalConfig.cs`.

use serde::Deserialize;

/// Global fracture scalars: model-shaping tuning constants (mirrors the engine's
/// `fracture::tuning`), asteroid-on-asteroid collision settings, and the default
/// impactor mass. Per-weapon and per-material knobs live on `WeaponConfig` /
/// `MaterialConfig`.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FractureGlobalConfig {
    // Model tuning constants (mirror engine FractureTuning).
    pub energy_scale: f32,
    pub reach_min: f32,
    pub reach_max: f32,
    pub vapor_eff: f32,
    pub break_perp: f32,
    pub fling_scale: f32,
    pub align_exponent: f32,
    pub spin_cap: f32,
    pub fragment_speed_max: f32,
    pub tumble_scale: f32,
    pub fragment_spin_max: f32,
    pub spin_profile_base: f32,
    pub split_stress_inherit: f32,

    // Impact-velocity → crack-speed coupling.
    pub crack_speed_ref_velocity: f32,
    pub crack_speed_vel_exponent: f32,

    // Asteroid-on-asteroid collision fracture.
    pub asteroid_blast_fraction: f32,
    pub asteroid_directionality: f32,
    /// 0 = crack direction is pure contact normal; 1 = pure relative velocity.
    pub asteroid_dir_spin: f32,
    pub asteroid_collision_threshold: f32,

    // Bullet impact.
    pub bullet_mass: f32,
}

impl Default for FractureGlobalConfig {
    fn default() -> Self {
        Self {
            energy_scale: 0.0001,
            reach_min: 0.1,
            reach_max: 0.96,
            vapor_eff: 0.4,
            break_perp: 1.0,
            fling_scale: 140.0,
            align_exponent: 1.6,
            spin_cap: 4.0,
            fragment_speed_max: 600.0,
            tumble_scale: 220.0,
            fragment_spin_max: 3.5,
            spin_profile_base: 0.3,
            split_stress_inherit: 1.0,
            crack_speed_ref_velocity: 600.0,
            crack_speed_vel_exponent: 0.5,
            asteroid_blast_fraction: 0.08,
            asteroid_directionality: 0.40,
            asteroid_dir_spin: 1.0,
            asteroid_collision_threshold: 20.0,
            bullet_mass: 10.0,
        }
    }
}
