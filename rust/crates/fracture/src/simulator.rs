//! Geometry + bookkeeping around the kernel — port of `FractureSimulator.cs`.
//! Pure: builds the graph snapshot, finds connected components over surviving
//! bonds, and turns them into fragment specs whose fling is derived from the
//! per-cell fling energy. No ECS dependency.

use crate::contract::{FractureInput, FragmentSpec};
use crate::geom::compute_inertia;
use crate::kernel::{step_front, CrackFront};
use crate::process::{FractureProcess, LivePiece};
use crate::properties::FractureState;
use crate::rng::Rng;
use crate::tuning;
use crate::types::{Bond, Cell, FracturableBody};
use glam::Vec2;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Graph snapshot for a live fracture.
// ---------------------------------------------------------------------------

/// Snapshot the bond graph: per-bond spin multiplier (1+spinFactor from body ω)
/// and per-cell bond adjacency. Port of `PrepareGraph`.
pub fn prepare_graph(body: &FracturableBody, spin_omega: f32) -> (Vec<f32>, Vec<Vec<usize>>) {
    let spin_mul = compute_spin_mul(
        &body.cells,
        &body.bonds,
        spin_omega,
        body.material.spin_pre_stress,
    );
    let adj = build_adjacency(body.cells.len(), &body.bonds);
    (spin_mul, adj)
}

/// Per-bond spin stress multiplier. Port of `ComputeSpinMul`.
pub fn compute_spin_mul(cells: &[Cell], bonds: &[Bond], omega: f32, spin_coeff: f32) -> Vec<f32> {
    let mut spin_mul = vec![1.0f32; bonds.len()];
    if bonds.is_empty() || spin_coeff <= 0.0 || omega == 0.0 {
        return spin_mul;
    }
    let mut rmax = 1e-3f32;
    for c in cells {
        rmax = rmax.max(c.centroid.length());
    }
    let w2 = omega * omega;
    for (k, b) in bonds.iter().enumerate() {
        let m = (cells[b.a].centroid + cells[b.b].centroid) * 0.5; // CoM = origin
        let r = m.length();
        if r < 1e-3 {
            continue;
        }
        let rad = m / r;
        let tan = Vec2::new(-rad.y, rad.x);
        let mut dir = cells[b.b].centroid - cells[b.a].centroid;
        let dl = dir.length();
        if dl > 1e-6 {
            dir /= dl;
        }
        let tangentiality = dir.dot(tan).abs();
        let base_p = tuning::SPIN_PROFILE_BASE;
        let profile = base_p + (1.0 - base_p) * (r / rmax);
        let sf = (spin_coeff * w2 * profile * tangentiality).clamp(0.0, tuning::SPIN_CAP);
        spin_mul[k] = 1.0 + sf;
    }
    spin_mul
}

fn build_adjacency(n: usize, bonds: &[Bond]) -> Vec<Vec<usize>> {
    let mut adj = vec![Vec::new(); n];
    for (k, b) in bonds.iter().enumerate() {
        adj[b.a].push(k);
        adj[b.b].push(k);
    }
    adj
}

// ---------------------------------------------------------------------------
// Connected components over the surviving bond graph (union-find, matches C#).
// ---------------------------------------------------------------------------

fn find(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]]; // path halving
        x = parent[x];
    }
    x
}

/// Label each cell with its component over surviving (unbroken, non-pulverized)
/// bonds. Pulverized cells get label -1. Returns `(labels, count)`. Port of
/// `FractureSimulator.ConnectedComponents`.
pub fn connected_components(
    n: usize,
    bonds: &[Bond],
    broken: &[bool],
    pulverized: &[bool],
) -> (Vec<i32>, usize) {
    let mut parent: Vec<usize> = (0..n).collect();
    for (k, b) in bonds.iter().enumerate() {
        if !broken[k] && !pulverized[b.a] && !pulverized[b.b] {
            let ra = find(&mut parent, b.a);
            let rb = find(&mut parent, b.b);
            parent[ra] = rb;
        }
    }
    let mut label = vec![-1i32; n];
    let mut count = 0usize;
    let mut comp = vec![-1i32; n];
    #[allow(clippy::needless_range_loop)] // i indexes parent/comp, not just pulverized
    for i in 0..n {
        if pulverized[i] {
            comp[i] = -1;
            continue;
        }
        let r = find(&mut parent, i);
        if label[r] < 0 {
            label[r] = count as i32;
            count += 1;
        }
        comp[i] = label[r];
    }
    (comp, count)
}

/// Number of surviving components, without materialising labels. Port of `CountComponents`.
pub fn count_components(n: usize, bonds: &[Bond], broken: &[bool], pulverized: &[bool]) -> usize {
    let mut parent: Vec<usize> = (0..n).collect();
    for (k, b) in bonds.iter().enumerate() {
        if !broken[k] && !pulverized[b.a] && !pulverized[b.b] {
            let ra = find(&mut parent, b.a);
            let rb = find(&mut parent, b.b);
            parent[ra] = rb;
        }
    }
    let mut count = 0;
    #[allow(clippy::needless_range_loop)] // i is both the index and the union-find node
    for i in 0..n {
        if !pulverized[i] && find(&mut parent, i) == i {
            count += 1;
        }
    }
    count
}

// ---------------------------------------------------------------------------
// Fragment construction.
// ---------------------------------------------------------------------------

/// Finalise a fracture: components over surviving bonds → fragment specs, with
/// fling derived from `fling_e`. Port of `BuildResult`.
pub fn build_result(
    body: &FracturableBody,
    input: &FractureInput,
    broken: &[bool],
    pulverized: &[bool],
    fling_e: &[f32],
    rng: &mut Rng,
    fling: bool,
) -> Vec<FragmentSpec> {
    let total_area: f32 = body.cells.iter().map(|c| c.area).sum();
    let (comp, comp_count) =
        connected_components(body.cells.len(), &body.bonds, broken, pulverized);

    let mut groups: Vec<Vec<usize>> = vec![Vec::new(); comp_count];
    for (i, &c) in comp.iter().enumerate() {
        if c >= 0 {
            groups[c as usize].push(i);
        }
    }

    let mut result = Vec::with_capacity(comp_count);
    for (c, group) in groups.iter().enumerate() {
        if group.is_empty() {
            continue;
        }
        let (spec, _remap) = build_component_spec(
            body, input, group, &comp, c as i32, broken, fling_e, total_area, rng, fling,
        );
        result.push(spec);
    }
    result
}

/// Build one component into a re-centred fragment body (+ old→new cell index
/// remap, for carrying live fronts across a split). Port of `BuildComponentSpec`.
#[allow(clippy::too_many_arguments)]
fn build_component_spec(
    body: &FracturableBody,
    input: &FractureInput,
    idxs: &[usize],
    comp: &[i32],
    label: i32,
    broken: &[bool],
    fling_e: &[f32],
    total_area: f32,
    rng: &mut Rng,
    fling: bool,
) -> (FragmentSpec, HashMap<usize, usize>) {
    let cells = &body.cells;
    let bonds = &body.bonds;
    let mat = body.material;
    let (sin, cos) = input.body_rotation.sin_cos();
    let body_pos = input.body_position;

    let mut area = 0.0f32;
    let mut weighted = 0.0f32;
    let mut cen = Vec2::ZERO;
    for &ci in idxs {
        area += cells[ci].area;
        weighted += cells[ci].area * cells[ci].density_mult;
        cen += cells[ci].centroid * cells[ci].area;
    }
    cen /= area;

    // A single detaching cell shrinks toward its centroid so it doesn't refill its socket.
    let shrink_cell = fling && idxs.len() == 1 && mat.detach_cell_scale > 0.0;
    let mut new_cells: Vec<Cell> = Vec::with_capacity(idxs.len());
    let mut cell_fling: Vec<f32> = Vec::with_capacity(idxs.len());
    let mut remap: HashMap<usize, usize> = HashMap::with_capacity(idxs.len());
    for (k, &ci) in idxs.iter().enumerate() {
        remap.insert(ci, k);
        cell_fling.push(fling_e[ci]);
        let src = &cells[ci];
        let cell_cen = src.centroid - cen;
        let mut local = Vec::with_capacity(src.local.len());
        if shrink_cell {
            let jitter = mat.detach_cell_jitter;
            for &v in &src.local {
                let s = mat.detach_cell_scale + (rng.next_f32() * 2.0 - 1.0) * jitter;
                local.push(cell_cen + (v - src.centroid) * s);
            }
        } else {
            for &v in &src.local {
                local.push(v - cen);
            }
        }
        new_cells.push(Cell {
            local,
            centroid: cell_cen,
            area: src.area,
            density_mult: src.density_mult,
            damage: src.damage * tuning::SPLIT_STRESS_INHERIT,
            role: src.role.clone(),
            fill_color: src.fill_color,
        });
    }

    // Keep only UNBROKEN bonds within the component; broken ones become cracks.
    let mut new_bonds: Vec<Bond> = Vec::new();
    for (bi, b) in bonds.iter().enumerate() {
        if !broken[bi] && comp[b.a] == label && comp[b.b] == label {
            new_bonds.push(Bond {
                a: remap[&b.a],
                b: remap[&b.b],
                edge_length: b.edge_length,
                strength: b.strength,
                strength_mult: b.strength_mult,
                stress: b.stress * tuning::SPLIT_STRESS_INHERIT,
                broken: false,
            });
        }
    }

    // Fragment mass = its density-weighted share of the body mass.
    let total_weighted: f32 = cells.iter().map(|c| c.area * c.density_mult).sum();
    let mass = if total_weighted > 1e-6 {
        input.body_mass * (weighted / total_weighted)
    } else {
        input.body_mass * (area / total_area)
    };
    let inertia = inertia_about(&new_cells, mass);
    let world_centroid = Vec2::new(
        cen.x * cos - cen.y * sin + body_pos.x,
        cen.x * sin + cen.y * cos + body_pos.y,
    );

    let (linear, angular) = if fling {
        derived_motion(input, world_centroid, &new_cells, &cell_fling, mass)
    } else {
        let r = world_centroid - input.body_position;
        (
            input.body_linear + Vec2::new(-input.body_angular * r.y, input.body_angular * r.x),
            input.body_angular,
        )
    };

    let spec = FragmentSpec {
        body: FracturableBody {
            cells: new_cells,
            bonds: new_bonds,
            material: mat,
            state: FractureState {
                rng_seed: rng.next_u32(),
            },
            fragile: body.fragile,
        },
        world_centroid,
        rotation: input.body_rotation,
        linear,
        angular,
        mass,
        inertia,
        area,
        is_debris: idxs.len() == 1 && area < 40.0,
    };
    (spec, remap)
}

/// Fragment linear + angular velocity derived from accumulated fling energy.
/// Port of `DerivedMotion`.
fn derived_motion(
    input: &FractureInput,
    world_centroid: Vec2,
    frag_cells: &[Cell],
    cell_fling: &[f32],
    mass: f32,
) -> (Vec2, f32) {
    let ekin: f32 = cell_fling.iter().sum();

    let r = world_centroid - input.body_position;
    let rot_vel = Vec2::new(-input.body_angular * r.y, input.body_angular * r.x);
    let mut spread = world_centroid - input.impact_point_world;
    let sl = spread.length();
    spread = if sl > 1e-4 { spread / sl } else { Vec2::X };

    let speed = (tuning::FLING_SCALE * (2.0 * ekin.max(0.0) / mass.max(1.0)).sqrt())
        .clamp(0.0, tuning::FRAGMENT_SPEED_MAX);
    let linear = input.body_linear + rot_vel + spread * speed;

    // Tumble: fling-weighted cell offset crossed with velocity.
    let mut off_local = Vec2::ZERO;
    let mut inertia = 0.0f32;
    if ekin > 1e-3 {
        for (k, c) in frag_cells.iter().enumerate() {
            let rc = c.centroid;
            off_local += rc * (cell_fling[k] / ekin);
            inertia += c.area * c.density_mult * rc.length_squared();
        }
    }
    let (s, c) = input.body_rotation.sin_cos();
    let off_world = Vec2::new(
        off_local.x * c - off_local.y * s,
        off_local.x * s + off_local.y * c,
    );
    let torque = off_world.x * linear.y - off_world.y * linear.x;
    let angular = input.body_angular
        + (torque / inertia.max(1e-3) * tuning::TUMBLE_SCALE)
            .clamp(-tuning::FRAGMENT_SPIN_MAX, tuning::FRAGMENT_SPIN_MAX);

    (linear, angular)
}

fn inertia_about(cells: &[Cell], mass: f32) -> f32 {
    let total: f32 = cells.iter().map(|c| c.area).sum();
    if total <= 0.0 {
        return 0.0;
    }
    let mut inertia = 0.0f32;
    for c in cells {
        let m = mass * (c.area / total);
        inertia += compute_inertia(&c.local, m) + m * c.centroid.length_squared();
    }
    inertia
}

// ---------------------------------------------------------------------------
// Mid-fracture split: partition a body that broke into ≥2 components.
// ---------------------------------------------------------------------------

/// Partition a body whose bonds broke into ≥2 components into fresh pieces,
/// carrying any still-active fronts into the piece they reach. Port of `SplitLive`.
pub fn split_live(
    body: &FracturableBody,
    input: &FractureInput,
    proc: &FractureProcess,
    rng: &mut Rng,
) -> Vec<LivePiece> {
    let cells = &body.cells;
    let bonds = &body.bonds;
    let total_area: f32 = cells.iter().map(|c| c.area).sum();

    let (comp, comp_count) =
        connected_components(cells.len(), bonds, &proc.broken, &proc.pulverized);
    let mut groups: Vec<Vec<usize>> = vec![Vec::new(); comp_count];
    for (i, &c) in comp.iter().enumerate() {
        if c >= 0 {
            groups[c as usize].push(i);
        }
    }

    // Continuer = component holding the most live wavefront energy.
    let mut energy_by_comp = vec![0.0f32; comp_count];
    for f in &proc.fronts {
        if !f.active() {
            continue;
        }
        for &fc in &f.frontier {
            if comp[fc] >= 0 {
                energy_by_comp[comp[fc] as usize] += f.energy[fc];
            }
        }
    }
    let mut continuer: i32 = -1;
    let mut best = 0.0f32;
    for (c, &e) in energy_by_comp.iter().enumerate() {
        if e > best {
            best = e;
            continuer = c as i32;
        }
    }

    let mut pieces = Vec::with_capacity(comp_count);
    for (c, group) in groups.iter().enumerate() {
        if group.is_empty() {
            continue;
        }
        let (spec, remap) = build_component_spec(
            body,
            input,
            group,
            &comp,
            c as i32,
            &proc.broken,
            &proc.fling_e,
            total_area,
            rng,
            c as i32 != continuer,
        );

        // Carry active fronts whose wavefront reaches into this component.
        let mut sub_fronts: Vec<CrackFront> = Vec::new();
        for f in &proc.fronts {
            if !f.active() {
                continue;
            }
            if f.frontier.iter().any(|&fc| comp[fc] == c as i32) {
                if let Some(sub) =
                    partition_front(f, &comp, c as i32, &remap, spec.body.cells.len())
                {
                    sub_fronts.push(sub);
                }
            }
        }

        let sub_proc = if !sub_fronts.is_empty() {
            let (spin_mul, adj) = prepare_graph(&spec.body, input.body_angular);
            Some(FractureProcess {
                fronts: sub_fronts,
                broken: vec![false; spec.body.bonds.len()],
                pulverized: vec![false; spec.body.cells.len()],
                emitted: vec![false; spec.body.cells.len()],
                fling_e: vec![0.0; spec.body.cells.len()],
                spin_mul,
                adj,
                impact_dir: proc.impact_dir,
                impact_point_world: proc.impact_point_world,
                directionality: proc.directionality,
                done: false,
            })
        } else {
            None
        };
        pieces.push(LivePiece {
            spec,
            process: sub_proc,
        });
    }
    pieces
}

/// Carve one component's slice out of a front. Port of `PartitionFront`.
fn partition_front(
    f: &CrackFront,
    comp: &[i32],
    label: i32,
    remap: &HashMap<usize, usize>,
    new_cell_count: usize,
) -> Option<CrackFront> {
    let mut energy = vec![0.0f32; new_cell_count];
    let mut processed = vec![-1.0f32; new_cell_count];
    let mut parent = vec![-1i32; new_cell_count];
    for (&old, &new) in remap {
        energy[new] = f.energy[old];
        processed[new] = f.processed[old];
        let p = f.parent[old];
        if p >= 0 {
            if let Some(&np) = remap.get(&(p as usize)) {
                parent[new] = np as i32;
            }
        }
    }
    let mut frontier = Vec::new();
    for &fc in &f.frontier {
        if comp[fc] == label {
            if let Some(&nk) = remap.get(&fc) {
                frontier.push(nk);
            }
        }
    }
    if frontier.is_empty() {
        return None;
    }
    Some(CrackFront {
        energy,
        processed,
        parent,
        frontier,
        impact_dir_local: f.impact_dir_local,
        directionality: f.directionality,
        brittleness: f.brittleness,
        blast_fraction: f.blast_fraction,
        steps_per_iteration: f.steps_per_iteration,
        frames_per_iteration: f.frames_per_iteration,
        frame_counter: f.frame_counter,
    })
}

/// Convenience driver (not in the C#, which drives fronts from `FractureCrackSystem`):
/// step every active front to completion, mutating the body + process in place.
/// Returns the indices of cells pulverized during the run. Handy for tests and as
/// the template for the Bevy `FixedUpdate` system.
pub fn drive_to_completion(body: &mut FracturableBody, proc: &mut FractureProcess) -> Vec<usize> {
    let mut pulv_out = Vec::new();
    let FractureProcess {
        fronts,
        broken,
        pulverized,
        fling_e,
        adj,
        spin_mul,
        ..
    } = proc;
    loop {
        let mut any = false;
        for f in fronts.iter_mut() {
            if f.active() {
                any = true;
                step_front(
                    f,
                    &mut body.cells,
                    &mut body.bonds,
                    adj,
                    spin_mul,
                    broken,
                    pulverized,
                    fling_e,
                    &body.material,
                    &mut pulv_out,
                );
            }
        }
        if !any {
            break;
        }
    }
    pulv_out
}
