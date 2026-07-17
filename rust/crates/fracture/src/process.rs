//! Multi-frame pacing + live state — port of `FractureProcess.cs`.

use crate::contract::FragmentSpec;
use crate::kernel::CrackFront;
use glam::Vec2;

/// Integer step/frame pacing for multi-frame crack propagation, derived from a
/// material's `crack_speed` (cells/sec).
#[derive(Clone, Copy, Debug)]
pub struct FractureTiming {
    pub steps_per_iteration: i32,
    pub frames_per_iteration: i32,
}

impl FractureTiming {
    pub const DEFAULT_FIXED_DT: f32 = 1.0 / 120.0;

    pub fn default_timing() -> Self {
        Self {
            steps_per_iteration: 2,
            frames_per_iteration: 1,
        }
    }

    /// Map a material `crack_speed` (cells/sec) to integer pacing at `fixed_dt`.
    pub fn from_crack_speed(crack_speed: f32, fixed_dt: f32) -> Self {
        let pops_per_step = crack_speed.max(0.0001) * fixed_dt.max(1e-4);
        if pops_per_step >= 1.0 {
            Self {
                steps_per_iteration: pops_per_step.round() as i32,
                frames_per_iteration: 1,
            }
        } else {
            Self {
                steps_per_iteration: 1,
                frames_per_iteration: (1.0 / pops_per_step).round().max(1.0) as i32,
            }
        }
    }
}

/// One piece produced by a mid-fracture split: a fragment body and, if it is
/// still cracking, the [`FractureProcess`] to attach on spawn.
#[derive(Clone, Debug)]
pub struct LivePiece {
    pub spec: FragmentSpec,
    pub process: Option<FractureProcess>,
}

/// Live, multi-frame fracture state on a body whose cracks are still spreading.
/// Holds one or more co-propagating [`CrackFront`]s sharing the accumulating
/// broken / pulverized / fling state and the persistent per-bond stress.
#[derive(Clone, Debug, Default)]
pub struct FractureProcess {
    pub fronts: Vec<CrackFront>,
    pub broken: Vec<bool>,     // over body.bonds
    pub pulverized: Vec<bool>, // over body.cells
    pub emitted: Vec<bool>,    // over body.cells — dust event sent?
    pub fling_e: Vec<f32>,     // over body.cells — shared fragment KE
    pub spin_mul: Vec<f32>,    // over body.bonds — 1 + spin factor
    pub adj: Vec<Vec<usize>>,  // per-cell bond adjacency

    // Fling snapshot (latest hit), fed to fragment construction on finalise.
    pub impact_dir: Vec2,
    pub impact_point_world: Vec2,
    pub directionality: f32,

    /// Set on finalise so the caller ignores it until the entity is destroyed.
    pub done: bool,
}
