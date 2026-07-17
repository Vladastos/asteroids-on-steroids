//! Physics-based destruction of cell/bond bodies — Rust port of
//! `GameEngine/Engine/Destruction/`.
//!
//! Design (unchanged from C#): a `FracturableBody` is a graph of convex `Cell`s
//! joined by `Bond`s. An impact deposits energy at a struck cell; a `CrackFront`
//! spreads it across the bond graph, breaking bonds and pulverising cells;
//! connected-component analysis over the surviving bonds yields the fragments.
//!
//! Pipeline (all pure — no Bevy, no renderer):
//!   1. [`compute_energy`] — reduced-mass impact energy (was `ComputeEnergy`).
//!   2. [`seed_process`] — build a [`FractureProcess`] seeded with one [`CrackFront`]
//!      (the non-ECS core of `FractureService.Seed`).
//!   3. [`step_front`] / [`drive_to_completion`] — advance the crack field
//!      (`FractureKernel.StepFront`).
//!   4. [`split_live`] / [`build_result`] — extract fragment bodies with derived
//!      motion (`FractureSimulator`).
//!
//! The Bevy `game` crate supplies the ECS glue (reads components, stores the
//! process, spawns fragments via `Commands`, applies knockback) — mirroring the
//! C# rule that the engine spawns no entities.

mod contract;
mod geom;
mod kernel;
mod process;
mod properties;
mod rng;
mod service;
mod simulator;
pub mod tuning;
mod types;
mod voronoi;

pub mod prelude {
    pub use crate::*;
}

pub use contract::{FractureInput, FractureResult, FragmentSpec};
pub use geom::{compute_inertia, contains_point, distance_to_polygon, nearest_cell};
pub use kernel::{step_front, CrackFront};
pub use process::{FractureProcess, FractureTiming, LivePiece};
pub use properties::{FractureProperties, FractureState};
pub use rng::Rng;
pub use service::{
    compute_energy, effective_directionality, fragile_vaporize_energy, seed_process, WeaponProfile,
};
pub use simulator::{
    build_result, compute_spin_mul, connected_components, count_components, drive_to_completion,
    prepare_graph, split_live,
};
pub use types::{Bond, Cell, FracturableBody, Rgba};
pub use voronoi::{
    build, build_asteroid, build_from_explicit_seeds, build_with_seeds, generate_convex,
};
