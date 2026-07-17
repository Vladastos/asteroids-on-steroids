//! Top-level entry points — port of `FractureService.cs`.
//!
//! `try_fracture` fractures atomically in one frame; `begin_fracture` seeds a
//! `FractureProcess` the caller advances over several frames. Both take `&mut`
//! the body (bonds/cells accumulate damage) and RETURN plain data — they never
//! spawn entities. The caller (a Bevy system) reads the result and issues
//! `Commands` to spawn fragments, exactly like the C# game layer.

use crate::contract::{FractureInput, FractureResult};
use crate::process::FractureProcess;
use crate::types::FracturableBody;
use glam::Vec2;

/// Impactor-side parameters (was `WeaponProfile` in `FractureService.cs`).
#[derive(Clone, Copy, Debug)]
pub struct WeaponProfile {
    /// 0 = omnidirectional splash … 1 = tight forward channel (avg'd with material).
    pub directionality: f32,
    /// Vaporize budget carved from each cell's energy → crater size.
    pub blast_fraction: f32,
    /// One-time recoil on the struck body, as a fraction of impactor speed.
    pub knockback: f32,
}

impl Default for WeaponProfile {
    fn default() -> Self {
        // TODO: match FractureService.WeaponProfile.Default values.
        Self { directionality: 0.5, blast_fraction: 0.15, knockback: 0.2 }
    }
}

/// Atomic, one-frame fracture. Port of `FractureService.TryFracture`.
///
/// Returns `FractureResult { fractured: false, .. }` if the impact was
/// sub-threshold (only accumulated damage/stress, no split).
#[allow(clippy::too_many_arguments)]
pub fn try_fracture(
    _body: &mut FracturableBody,
    _impact_point: Vec2,
    _impact_dir: Vec2,
    _normal_speed: f32,
    _impactor_mass: f32,
    _weapon: &WeaponProfile,
    _body_pos: Vec2,
    _body_rot: f32,
    _body_linear: Vec2,
    _body_angular: f32,
    _body_mass: f32,
    _body_inertia: f32,
) -> FractureResult {
    todo!("port FractureService.TryFracture: reduced-mass energy → FractureInput → simulator")
}

/// Seed a multi-frame fracture. Port of `FractureService.BeginFracture`.
/// Returns `None` if sub-threshold, else the process for the caller to store on
/// the entity and advance each fixed step via [`crate::split_live`].
#[allow(clippy::too_many_arguments)]
pub fn begin_fracture(
    _body: &mut FracturableBody,
    _impact_point: Vec2,
    _impact_dir: Vec2,
    _normal_speed: f32,
    _impactor_mass: f32,
    _weapon: &WeaponProfile,
    // ... body motion params as in try_fracture ...
) -> Option<(FractureProcess, FractureInput)> {
    todo!("port FractureService.BeginFracture: seed CrackFront over the struck cell")
}

/// Reduced-mass impact energy → fracture-energy units. Port of the private
/// `FractureService` energy helper (around line 209 in the C#).
pub(crate) fn impact_energy(
    _impact_point: Vec2,
    _dir: Vec2,
    _normal_speed: f32,
    _impactor_mass: f32,
    _body_pos: Vec2,
    _m_body: f32,
    _i_body: f32,
    _restitution: f32,
) -> f32 {
    todo!("port reduced-mass KE → EnergyScale conversion")
}
