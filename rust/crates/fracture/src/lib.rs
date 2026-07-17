//! Physics-based destruction of cell/bond bodies — Rust port of
//! `GameEngine/Engine/Destruction/`.
//!
//! Design (unchanged from C#): a `FracturableBody` is a graph of convex `Cell`s
//! joined by `Bond`s. An impact deposits energy at a struck cell; a `CrackFront`
//! spreads it across the bond graph, breaking bonds and pulverising cells;
//! connected-component analysis over the surviving bonds yields the fragments.
//!
//! Two entry points mirror the C# `FractureService`:
//!   * [`try_fracture`]  — atomic, one-frame (was `FractureService.TryFracture`)
//!   * [`begin_fracture`] — seeds a [`FractureProcess`] the caller advances over
//!     several frames (was `FractureService.BeginFracture` + `FractureCrackSystem`)
//!
//! Nothing here touches Bevy or a renderer. `Rgba`/`role` on a cell are carried
//! through splits but ignored by the physics (see [`types::Cell`]).

mod contract;
mod kernel;
mod process;
mod properties;
mod service;
mod simulator;
mod types;

pub use contract::{FractureInput, FractureResult, FragmentSpec};
pub use kernel::CrackFront;
pub use process::{FractureProcess, FractureTiming, LivePiece};
pub use properties::{FractureProperties, FractureState};
pub use service::{begin_fracture, try_fracture, WeaponProfile};
pub use simulator::{
    build_result, connected_components, count_components, prepare_graph, split_live,
};
pub use types::{Bond, Cell, FracturableBody, Rgba};
