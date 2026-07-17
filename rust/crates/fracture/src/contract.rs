//! Fracture I/O — port of `FractureContract.cs`.

use crate::types::FracturableBody;
use glam::Vec2;

/// Inputs to one fracture evaluation. Directions are unit vectors in world space,
/// angles in radians. The service computes the energy terms; the simulator does
/// the geometry + physics.
#[derive(Clone, Copy, Debug)]
pub struct FractureInput {
    pub impact_point_world: Vec2,
    pub impact_dir: Vec2,
    /// Effective (weapon + material)/2: 0 = splash … 1 = forward channel.
    pub directionality: f32,
    /// Weapon: carves the vaporize budget from each cell's energy.
    pub blast_fraction: f32,
    pub body_position: Vec2,
    pub body_rotation: f32,
    pub body_linear: Vec2,
    pub body_angular: f32,
    pub body_mass: f32,
}

/// One resulting body from a fracture: a connected component of cells, re-centred
/// and ready for the caller to spawn as an entity.
#[derive(Clone, Debug)]
pub struct FragmentSpec {
    pub body: FracturableBody,
    pub world_centroid: Vec2,
    pub rotation: f32,
    pub linear: Vec2,
    pub angular: f32,
    pub mass: f32,
    pub inertia: f32,
    pub area: f32,
    /// Single tiny cell → visual particle, not a physics body.
    pub is_debris: bool,
}

/// Output of a one-shot fracture (the multi-frame path emits pieces incrementally).
#[derive(Clone, Debug, Default)]
pub struct FractureResult {
    pub fractured: bool,
    pub fragments: Vec<FragmentSpec>,
    pub impact_point_world: Vec2,
}
