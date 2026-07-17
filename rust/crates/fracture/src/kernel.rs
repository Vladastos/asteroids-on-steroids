//! The conservative crack-propagation kernel — port of `FractureKernel.cs`
//! (`CrackFront` + `StepFront`). Pure: operates on plain arrays, no ECS.

use crate::process::FractureTiming;
use crate::properties::FractureProperties;
use crate::tuning::{self, lerp};
use crate::types::{Bond, Cell};
use glam::Vec2;

/// One impact's live crack field over a body's bond graph. A cell splits its
/// incoming energy into channels (break / vaporize / fling / transmit). Multi-frame
/// fracture advances several co-propagating fronts sharing the body's
/// broken / pulverized / fling state.
#[derive(Clone, Debug, Default)]
pub struct CrackFront {
    /// Per cell: incoming energy for this front.
    pub energy: Vec<f32>,
    /// Per cell: -1 = not processed; ≥0 = processed at that energy.
    pub processed: Vec<f32>,
    /// Per cell: who delivered its energy → local flow direction (-1 = none).
    pub parent: Vec<i32>,
    pub frontier: Vec<usize>,
    pub impact_dir_local: Vec2,
    pub directionality: f32,
    pub brittleness: f32,
    pub blast_fraction: f32,

    // Per-front pacing (material crack_speed × the hit's velocity factor).
    pub steps_per_iteration: i32,
    pub frames_per_iteration: i32,
    pub frame_counter: i32,
}

impl CrackFront {
    pub fn active(&self) -> bool {
        !self.frontier.is_empty()
    }

    /// Seed a front at the struck cell over `energy` (length = cell count).
    /// Port of `CrackFront.Seed`.
    #[allow(clippy::too_many_arguments)]
    pub fn seed(
        mut energy: Vec<f32>,
        struck: usize,
        start_energy: f32,
        impact_dir_local: Vec2,
        directionality: f32,
        brittleness: f32,
        blast_fraction: f32,
        crack_speed: f32,
        normal_speed: f32,
    ) -> Self {
        let timing = FractureTiming::from_crack_speed(
            crack_speed * tuning::crack_speed_factor(normal_speed),
            FractureTiming::DEFAULT_FIXED_DT,
        );
        let n = energy.len();
        energy[struck] = start_energy;
        Self {
            processed: vec![-1.0; n],
            parent: vec![-1; n],
            frontier: vec![struck],
            energy,
            impact_dir_local,
            directionality,
            brittleness,
            blast_fraction,
            steps_per_iteration: timing.steps_per_iteration,
            frames_per_iteration: timing.frames_per_iteration,
            frame_counter: 0,
        }
    }
}

#[inline]
fn route_fling(j: usize, amount: f32, pulverized: &[bool], fling_e: &mut [f32]) {
    if amount <= 0.0 {
        return;
    }
    if !pulverized[j] {
        fling_e[j] += amount; // a vaporised cell just disperses it as dust
    }
}

/// Advance a front by one frontier-pop. Port of `FractureKernel.StepFront`.
/// `cells`/`bonds` accumulate damage/stress; `broken`/`pulverized`/`fling_e` are
/// SHARED across co-propagating fronts. Newly-vaporised cells append to `pulv_out`.
#[allow(clippy::too_many_arguments)]
pub fn step_front(
    f: &mut CrackFront,
    cells: &mut [Cell],
    bonds: &mut [Bond],
    adj: &[Vec<usize>],
    spin_mul: &[f32],
    broken: &mut [bool],
    pulverized: &mut [bool],
    fling_e: &mut [f32],
    mat: &FractureProperties,
    pulv_out: &mut Vec<usize>,
) {
    // Copy scalars, then split-borrow the per-cell arrays off `f`.
    let impact_dir_local = f.impact_dir_local;
    let brittleness = f.brittleness;
    let blast_fraction = f.blast_fraction;
    let directionality = f.directionality;
    let energy = &mut f.energy;
    let processed = &mut f.processed;
    let parent = &mut f.parent;
    let frontier = &mut f.frontier;

    if frontier.is_empty() {
        return;
    }

    // Pop the highest-energy frontier cell (each cell processed once at its peak).
    let mut mi = 0;
    for k in 1..frontier.len() {
        if energy[frontier[k]] > energy[frontier[mi]] {
            mi = k;
        }
    }
    let i = frontier[mi];
    let e = energy[i];
    frontier.remove(mi);

    if processed[i] >= 0.0 {
        route_fling(i, e, pulverized, fling_e); // settled: surplus → fling
        return;
    }
    processed[i] = e;

    // Brittleness sets the outward/local split. Local dump → fling (1-blast) +
    // vaporise (blast); vaporisation accumulates comminution toward the threshold.
    let mut transmit;
    if pulverized[i] {
        transmit = e; // already dust → pure conduit
    } else {
        let tf = lerp(tuning::REACH_MIN, tuning::REACH_MAX, brittleness);
        transmit = tf * e;
        let dump = e - transmit;
        fling_e[i] += (1.0 - blast_fraction) * dump;

        let vaporise_energy = blast_fraction * dump;
        if vaporise_energy > 0.0 {
            let threshold =
                mat.cell_toughness * cells[i].area * cells[i].density_mult * mat.density;
            let comminution = vaporise_energy.min((threshold - cells[i].damage).max(0.0));
            cells[i].damage += comminution;
            let surplus = vaporise_energy - comminution;
            if cells[i].damage >= threshold {
                pulverized[i] = true;
                pulv_out.push(i);
                transmit += tuning::VAPOR_EFF * surplus; // penetration; rest → heat (lost)
            }
        }
    }

    // Local flow direction: from the cell that delivered this energy.
    let mut flow = impact_dir_local;
    let par = parent[i];
    if par >= 0 {
        let fd = cells[i].centroid - cells[par as usize].centroid;
        let fl = fd.length();
        if fl > 1e-6 {
            flow = fd / fl;
        }
    }

    // Weight each intact outgoing bond: conduct weight wc (alignment) + damage
    // weight wd (perpendicularity). (bond_idx, other_cell, wc, wd).
    let mut out: Vec<(usize, usize, f32, f32)> = Vec::with_capacity(adj[i].len());
    let mut w = 0.0f32;
    let mut sum_wc = 0.0f32;
    for &bk in &adj[i] {
        if broken[bk] {
            continue;
        }
        let j = if bonds[bk].a == i {
            bonds[bk].b
        } else {
            bonds[bk].a
        };
        if pulverized[j] {
            continue;
        }
        let mut d = cells[j].centroid - cells[i].centroid;
        let dl = d.length();
        if dl > 1e-6 {
            d /= dl;
        }
        let align = d.dot(flow);
        let aa = align.abs();
        let wc = lerp(
            1.0,
            align.max(0.0).powf(tuning::ALIGN_EXPONENT),
            directionality,
        );
        let wd = lerp(aa, 1.0 - aa, tuning::BREAK_PERP);
        out.push((bk, j, wc, wd));
        w += wc + wd;
        sum_wc += wc;
    }

    // Isolated/last cell: transmit has nowhere to go → this cell's fling.
    if out.is_empty() || w <= 1e-9 {
        route_fling(i, transmit, pulverized, fling_e);
        return;
    }

    // DAMAGE pass: each bond absorbs transmit·wd/W but only CONSUMES what it needs
    // to break; over-damage is recovered and continues forward. Spin multiplies the
    // damage per unit energy, not the bond strength.
    let mut recovered = 0.0f32;
    for &(bk, _j, _wc, wd) in &out {
        let absorb = transmit * wd / w;
        let str_ = bonds[bk].strength;
        let spin = spin_mul[bk].max(1e-4);
        let need = ((str_ - bonds[bk].stress) / spin).max(0.0);
        let consumed = absorb.min(need);
        bonds[bk].stress += consumed * spin;
        if bonds[bk].stress >= str_ - 1e-4 {
            broken[bk] = true;
            bonds[bk].broken = true;
        }
        recovered += absorb - consumed;
    }

    // FORWARD = conduct budget + recovered over-damage, distributed by conduct weight.
    let fwd_total = transmit * (sum_wc / w) + recovered;
    if sum_wc > 1e-9 {
        for &(_bk, j, wc, _wd) in &out {
            let fwd = fwd_total * wc / sum_wc;
            if fwd <= 0.0 {
                continue;
            }
            if processed[j] < 0.0 && fwd > energy[j] {
                energy[j] = fwd;
                parent[j] = i as i32;
                frontier.push(j);
            } else {
                route_fling(j, fwd, pulverized, fling_e);
            }
        }
    } else {
        route_fling(i, fwd_total, pulverized, fling_e);
    }
}
