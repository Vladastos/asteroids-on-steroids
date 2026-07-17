//! Port of `GameConfig/Models/MaterialConfig.cs`.

use serde::Deserialize;

/// All tuneable fields of `fracture::FractureProperties` plus `CrackDirectionality`.
/// Maps 1-to-1 with the engine struct; the game layer converts via an extension method.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MaterialConfig {
    pub brittleness: f32,
    pub toughness: f32,
    /// Coefficient of restitution for the dissipated energy: only (1 − e²) of the
    /// contact energy couples into fracture (the rest bounces back).
    pub restitution: f32,
    /// Stress/sec at which accumulated per-bond Stress relaxes when not being hit.
    pub relax_rate: f32,
    /// 0 = isotropic shatter, 1 = clean cleavage along grain.
    pub crack_directionality: f32,
    /// Cells/second the crack front advances (multi-frame pacing).
    pub crack_speed: f32,
    pub grain_area: f32,
    pub min_fragment_area: f32,
    pub density: f32,
    /// Vaporise threshold per unit cell mass.
    pub cell_toughness: f32,
    pub spin_pre_stress: f32,
    pub detach_cell_scale: f32,
    pub detach_cell_jitter: f32,
}

impl Default for MaterialConfig {
    fn default() -> Self {
        Self {
            brittleness: 0.6,
            toughness: 16.0,
            restitution: 0.30,
            relax_rate: 200.0,
            crack_directionality: 0.35,
            crack_speed: 240.0,
            grain_area: 1500.0,
            min_fragment_area: 180.0,
            density: 1.4,
            cell_toughness: 0.5,
            spin_pre_stress: 0.12,
            detach_cell_scale: 0.90,
            detach_cell_jitter: 0.02,
        }
    }
}
