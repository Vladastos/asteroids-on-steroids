//! Port of `GameConfig/Models/AsteroidConfig.cs`.

use serde::Deserialize;

/// Asteroid variant config. Either references an authored shape file
/// (`shape.is_some()`) or describes a procedural generator. The loader checks
/// `shape` first.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AsteroidConfig {
    pub shape: Option<String>,
    pub procedural: Option<ProceduralAsteroidConfig>,
    pub material: String,
    /// `[min, max]` uniform scale applied at spawn.
    pub size_range: [f32; 2],
    /// `[min, max]` initial angular speed (rad/s).
    pub spin_range: [f32; 2],
    pub density_mult: f32,
    /// Abstract cost per unit of size_mult for the wave budget system.
    pub base_cost: i32,
    /// `[min, max]` initial speed (px/s); direction is inward from the spawn border.
    pub speed_range: [f32; 2],
    /// First wave this type can appear on.
    pub unlock_wave: i32,
    pub vortex_response: Option<VortexResponseConfig>,
}

impl Default for AsteroidConfig {
    fn default() -> Self {
        Self {
            shape: None,
            procedural: None,
            material: "rock".into(),
            size_range: [0.8, 1.2],
            spin_range: [0.0, 1.0],
            density_mult: 1.0,
            base_cost: 3,
            speed_range: [30.0, 90.0],
            unlock_wave: 1,
            vortex_response: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProceduralAsteroidConfig {
    pub base_radius: f32,
    /// `[min, max]` vertex count for the outline polygon.
    pub vertex_count: [i32; 2],
    /// Noise amplitude as a fraction of `base_radius`.
    pub roughness: f32,
    pub noise_frequency: f32,
    /// Probability `[0,1]` of an inward dent per vertex.
    pub concavity_bias: f32,
    /// Lloyd (centroidal-Voronoi) relaxation passes applied to the seeds before
    /// tessellation.
    pub relax_iterations: i32,

    // Material clusters.
    pub cluster_count: i32,
    /// `[0,1]` radial position of cluster centres: 1 = centre, 0 = surface.
    pub cluster_centrality: f32,
    /// `[min,max]` per-cluster reach as a fraction of the body radius.
    pub cluster_spread: [f32; 2],
    pub bond_gain: f32,
    pub density_gain: f32,
}

impl Default for ProceduralAsteroidConfig {
    fn default() -> Self {
        Self {
            base_radius: 80.0,
            vertex_count: [10, 16],
            roughness: 0.22,
            noise_frequency: 3.0,
            concavity_bias: 0.05,
            relax_iterations: 2,
            cluster_count: 0,
            cluster_centrality: 0.5,
            cluster_spread: [0.2, 0.4],
            bond_gain: 2.0,
            density_gain: 1.5,
        }
    }
}

/// Per-entity vortex multipliers sampled uniformly from the ranges at spawn.
/// Negative values make the entity resist or oppose the vortex direction.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VortexResponseConfig {
    pub centripetal_range: [f32; 2],
    pub tangential_range: [f32; 2],
}

impl Default for VortexResponseConfig {
    fn default() -> Self {
        Self {
            centripetal_range: [1.0, 1.0],
            tangential_range: [1.0, 1.0],
        }
    }
}
