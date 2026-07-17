//! Core data — port of `Cell.cs`, `Bond.cs`, `FracturableBody.cs`.

use crate::properties::{FractureProperties, FractureState};
use glam::Vec2;

/// Rendering-only colour carried through fracture. Defined locally so this crate
/// stays free of any renderer/Bevy dependency; the `game` crate converts to
/// `bevy::Color` at draw time. (In C# this was `Engine.Rendering.Color` on `Cell`.)
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8, // a == 0 → not yet baked
}

/// One convex Voronoi cell in body-local (centroid-relative) space.
/// The union of a body's cells is its (possibly concave) silhouette.
#[derive(Clone, Debug)]
pub struct Cell {
    /// Convex polygon, body-local vertices.
    pub local: Vec<Vec2>,
    pub centroid: Vec2,
    /// |area| of the cell (px²).
    pub area: f32,
    /// Per-cell density multiplier (1 = material density). Armor = dense + heavy.
    pub density_mult: f32,
    /// Accumulated comminution toward the vaporise threshold (fatigue; decays by RelaxRate).
    pub damage: f32,
    /// Functional role from the shape editor (cockpit, cannon, …); `None` = generic.
    pub role: Option<String>,
    /// Baked fill colour — rendering only, ignored by physics.
    pub fill_color: Rgba,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            local: Vec::new(),
            centroid: Vec2::ZERO,
            area: 0.0,
            density_mult: 1.0,
            damage: 0.0,
            role: None,
            fill_color: Rgba::default(),
        }
    }
}

/// A cohesive bond between two adjacent cells sharing a Voronoi edge.
/// Index-based (`a`/`b` reference `FracturableBody::cells`) — this is exactly
/// why the graph ports to Rust without borrow-checker friction.
#[derive(Clone, Copy, Debug, Default)]
pub struct Bond {
    pub a: usize,
    pub b: usize,
    pub edge_length: f32,
    /// Stress to break = edge_length × Toughness × strength_mult. Set once, never mutated.
    pub strength: f32,
    /// Per-bond strength multiplier (preserves clusters across live toughness edits).
    pub strength_mult: f32,
    /// Runtime damage accumulator; breaks at `stress >= strength`, decays by RelaxRate.
    pub stress: f32,
    /// Permanent crack flag — set on break, never cleared (distinct from relaxing `stress`).
    pub broken: bool,
}

/// A pre-fractured body: convex cells joined by a bond graph. Always a single
/// connected component (= one rigid body / one compound collider).
#[derive(Clone, Debug)]
pub struct FracturableBody {
    pub cells: Vec<Cell>,
    /// Current adjacency; shrinks as cracks form.
    pub bonds: Vec<Bond>,
    pub material: FractureProperties,
    pub state: FractureState,
    /// Whole body vaporises on any fracture (all cells below MinFragmentArea, non-cockpit).
    pub fragile: bool,
}
