//! Crack field — port of `FractureKernel.cs` (`CrackFront`).

use glam::Vec2;

/// One impact's live crack field over a body's bond graph. A cell splits its
/// incoming energy into channels that sum to the input (break / vaporize / fling /
/// transmit) — nothing vanishes. Multi-frame fracture advances several
/// co-propagating fronts a few pops per frame, all sharing the body's
/// broken / pulverized / bond-stress / fling state.
#[derive(Clone, Debug, Default)]
pub struct CrackFront {
    /// Per cell: incoming energy for this front.
    pub energy: Vec<f32>,
    /// Per cell: -1 = not processed; ≥0 = processed at that energy.
    pub processed: Vec<f32>,
    /// Per cell: who delivered its energy → local flow direction.
    pub parent: Vec<i32>,
    pub frontier: Vec<usize>,
    /// Flow at the struck cell (no parent).
    pub impact_dir_local: Vec2,
    pub directionality: f32,
    pub brittleness: f32,
    pub blast_fraction: f32,

    // Per-front pacing: each hit's crack advances on its own clock.
    pub steps_per_iteration: i32,
    pub frames_per_iteration: i32,
    pub frame_counter: i32,
}

impl CrackFront {
    pub fn active(&self) -> bool {
        !self.frontier.is_empty()
    }

    /// Seed a front at the struck cell. Port of `CrackFront.Seed`.
    #[allow(clippy::too_many_arguments)]
    pub fn seed(
        _energy: Vec<f32>,
        _struck: usize,
        _start_energy: f32,
        _impact_dir_local: Vec2,
        _directionality: f32,
        _brittleness: f32,
        _blast_fraction: f32,
        _crack_speed: f32,
        _normal_speed: f32,
    ) -> Self {
        todo!("port CrackFront.Seed + FractureTiming::from_crack_speed pacing")
    }

    /// Advance the front by its per-iteration step budget. Port of the kernel's
    /// frontier-pop loop (energy split → break bonds → enqueue neighbours).
    pub fn step(&mut self /* body, shared broken/pulverized/fling state */) {
        todo!("port FractureKernel frontier advance")
    }
}
