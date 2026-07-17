//! Pure fracture entry math — the non-ECS half of `FractureService.cs`.
//!
//! The C# `FractureService` is coupled to the ECS (`World`/`Entity`, reads
//! Transform/RigidBody/Velocity, adds a `FractureProcess` component, applies
//! knockback). That glue belongs in the `game` crate's Bevy system. Here we port
//! the pure pieces it calls: the impact-energy model, the directionality blend,
//! the fragile vaporize budget, and `seed_process` (the "fresh process" branch of
//! `Seed`, minus all World access).

use crate::geom::nearest_cell;
use crate::kernel::CrackFront;
use crate::process::FractureProcess;
use crate::simulator::prepare_graph;
use crate::tuning;
use crate::types::FracturableBody;
use glam::Vec2;

/// Per-shot weapon/impactor parameters (port of `WeaponProfile`).
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
        Self { directionality: 0.4, blast_fraction: 0.3, knockback: 0.01 }
    }
}

/// Weapon impact-cone focus blended with the material's grain/cleavage bias
/// (arithmetic mean). Port of `EffectiveDirectionality`.
pub fn effective_directionality(weapon_dir: f32, material_cleavage: f32) -> f32 {
    (weapon_dir + material_cleavage) * 0.5
}

/// Contact-impulse dissipated energy: effective mass (impactor + body linear +
/// rotational lever arm (r×n)²/I), then E = ½·m_eff·v_n²·(1−e²) scaled to
/// fracture-energy units. Port of `FractureService.ComputeEnergy`.
#[allow(clippy::too_many_arguments)]
pub fn compute_energy(
    impact_point: Vec2,
    dir: Vec2,
    normal_speed: f32,
    impactor_mass: f32,
    body_pos: Vec2,
    m_body: f32,
    i_body: f32,
    restitution: f32,
) -> f32 {
    let r = impact_point - body_pos;
    let rxn = r.x * dir.y - r.y * dir.x; // (r × n) z
    let mut inv_mass = (if impactor_mass > 0.0 { 1.0 / impactor_mass } else { 0.0 })
        + (if m_body > 0.0 { 1.0 / m_body } else { 0.0 });
    if i_body > 1e-6 {
        inv_mass += rxn * rxn / i_body;
    }
    let m_eff = if inv_mass > 1e-9 { 1.0 / inv_mass } else { m_body };
    let e = restitution.clamp(0.0, 0.95);
    tuning::ENERGY_SCALE * 0.5 * m_eff * normal_speed * normal_speed * (1.0 - e * e)
}

/// Total energy needed to vaporize a fragile body whole (flood budget). Mirrors the
/// fragile branch of `FractureService.Seed`. `vapor_eff` is `FractureTuning.VaporEff`.
pub fn fragile_vaporize_energy(body: &FracturableBody) -> f32 {
    let mat = &body.material;
    let mut thr = 0.0f32;
    for c in &body.cells {
        thr += mat.cell_toughness * c.area * c.density_mult * mat.density;
    }
    thr / tuning::VAPOR_EFF.max(0.05) * 4.0 + 1.0
}

/// Build a fresh `FractureProcess` seeded with one crack front — the pure core of
/// `FractureService.Seed` (fresh-process branch). The caller (a Bevy system)
/// resolves the struck cell / applies knockback / stores this as a component and
/// advances it. `struck < 0` → resolve by nearest cell.
///
/// `e` is the pre-computed impact energy from [`compute_energy`]; the fragile
/// override replaces it as in the C#.
#[allow(clippy::too_many_arguments)]
pub fn seed_process(
    body: &FracturableBody,
    struck: i32,
    impact_point_world: Vec2,
    body_pos: Vec2,
    body_rotation: f32,
    body_angular: f32,
    dir_world: Vec2,
    e: f32,
    weapon: &WeaponProfile,
    normal_speed: f32,
) -> FractureProcess {
    let dir = if dir_world.length_squared() > 1e-8 {
        dir_world.normalize()
    } else {
        Vec2::X
    };

    let cell = if struck < 0 || struck as usize >= body.cells.len() {
        nearest_cell(body, body_pos, body_rotation, impact_point_world)
    } else {
        struck as usize
    };

    let (energy_start, effective_dir, blast) = if body.fragile {
        (fragile_vaporize_energy(body), 0.0, 1.0)
    } else {
        (
            e,
            effective_directionality(weapon.directionality, body.material.crack_directionality),
            weapon.blast_fraction,
        )
    };

    // Body-local impact direction.
    let (rs, rc) = body_rotation.sin_cos();
    let dir_local = Vec2::new(dir.x * rc + dir.y * rs, -dir.x * rs + dir.y * rc);

    let (spin_mul, adj) = prepare_graph(body, body_angular);

    // Latent cracks: bonds broken by a prior hit start already broken (permanent).
    let broken: Vec<bool> = body.bonds.iter().map(|b| b.broken).collect();

    let front = CrackFront::seed(
        vec![0.0; body.cells.len()],
        cell,
        energy_start,
        dir_local,
        effective_dir,
        body.material.brittleness,
        blast,
        body.material.crack_speed,
        normal_speed,
    );

    FractureProcess {
        fronts: vec![front],
        broken,
        pulverized: vec![false; body.cells.len()],
        emitted: vec![false; body.cells.len()],
        fling_e: vec![0.0; body.cells.len()],
        spin_mul,
        adj,
        impact_dir: dir,
        impact_point_world,
        directionality: effective_dir,
        done: false,
    }
}
