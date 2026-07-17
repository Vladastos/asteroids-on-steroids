//! Material params — port of `Engine/Components/FractureProperties.cs` and
//! `FractureState.cs`. Kept here (not in `game-core`) because the simulator
//! reads them directly and they carry no ECS/Bevy meaning.

/// Immutable material description for a cell/bond body. Shareable across entities;
/// runtime-mutable accumulation lives in [`FractureState`]. See the C# doc comments
/// for the full physical meaning of each knob.
#[derive(Clone, Copy, Debug)]
pub struct FractureProperties {
    /// bond.strength = shared_edge_len × toughness × strength_mult.
    pub toughness: f32,
    /// Coefficient of restitution; only (1 − e²) of contact energy becomes input E.
    pub restitution: f32,
    /// Stress-per-second each bond's `stress` relaxes when not hit. 0 = never heals.
    pub relax_rate: f32,
    /// [0 ductile … 1 brittle]. Splits cell energy into local dump vs. transmitted crack.
    pub brittleness: f32,
    /// Cells/sec the crack front advances (multi-frame pacing).
    pub crack_speed: f32,
    /// Target cell area (px²) at tessellation — the material "grain".
    pub grain_area: f32,
    /// Area (px²) below which a piece becomes visual debris, not a live body.
    pub min_fragment_area: f32,
    /// Mass per unit area. cell_mass = area × density_mult × density.
    pub density: f32,
    /// Vaporise threshold per unit cell mass.
    pub cell_toughness: f32,
    /// Gain on how strongly body spin ω pre-stresses tangential rim bonds.
    pub spin_pre_stress: f32,
    /// [0 isotropic … 1 clean cleavage]. Grain guidance of crack direction.
    pub crack_directionality: f32,
    /// Mean vertex contraction when a lone cell detaches (avoids overlap with the hole).
    pub detach_cell_scale: f32,
    /// Per-vertex ± variance on `detach_cell_scale`.
    pub detach_cell_jitter: f32,
}

/// Accumulated sub-threshold fracture state on a body (port of `FractureState.cs`).
/// Fill in fields as you port; kept separate from the immutable material.
#[derive(Clone, Copy, Debug, Default)]
pub struct FractureState {
    // TODO: port fields from Engine/Components/FractureState.cs
    pub accumulated_damage: f32,
}
