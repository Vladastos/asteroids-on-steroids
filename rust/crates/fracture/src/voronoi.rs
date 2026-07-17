//! Builds a [`FracturableBody`] by pre-fracturing a convex region into Voronoi
//! cells and bonding adjacent cells. Port of `VoronoiTessellator.cs`.
//!
//! Algorithm (matches the design prototype, docs/destruction_engine_spec.md §4.1):
//!   1. Scatter seeds on a jittered grid (spacing from the material grain).
//!   2. Every seed's Voronoi cell = the bound clipped by the perpendicular
//!      bisector against every OTHER seed (so kept cells keep their true size).
//!   3. Keep cells whose seed passes the membership predicate — concavity is
//!      just absent cells (a crater is a region of removed cells).
//!   4. Re-centre to the collective centroid (`Transform.Position` = world centroid).
//!   5. Bond cells that share a Voronoi edge; bond strength ∝ shared edge length.
//!
//! Note: builds the cell/bond graph only. The collision collider (one convex
//! `collision::Polygon` per cell, zipped into a `collision::Compound`) is a
//! one-line glue concern left to the `game` crate — porting it here would add a
//! `fracture → collision` dependency for no algorithmic benefit.

use crate::geom::{clip_convex_by_half_plane, compute_area, compute_centroid, contains_point};
use crate::properties::{FractureProperties, FractureState};
use crate::rng::Rng;
use crate::types::{Bond, Cell, FracturableBody};
use glam::Vec2;

const MIN_CELL_AREA: f32 = 2.0; // drop degenerate slivers
const MIN_SHARED_EDGE: f32 = 1.5; // below this no bond is formed
const COLLINEAR_TOL: f32 = 0.7; // px; shared-edge collinearity tolerance

// ---------------------------------------------------------------------------
// Random convex outline generation (Valtr's algorithm).
// ---------------------------------------------------------------------------

/// A uniformly random convex polygon with exactly `sides` vertices, centred at
/// the origin with clockwise winding and mean vertex distance ≈ `radius`.
/// Port of `PolygonUtils.GenerateConvex`.
pub fn generate_convex(
    sides: usize,
    radius: f32,
    rng: &mut Rng,
    fault_count: usize,
) -> (Vec<Vec2>, Vec<f32>) {
    assert!(sides >= 3, "need at least 3 sides");
    let verts = random_convex_polygon(sides, rng);

    let (_, mut centred) = crate::geom::recenter_vertices(&verts);
    let mean_r: f32 = centred.iter().map(|v| v.length()).sum::<f32>() / centred.len() as f32;
    let scale = if mean_r > 1e-6 { radius / mean_r } else { 1.0 };
    for v in &mut centred {
        *v *= scale;
    }

    // Clockwise winding (engine convention, Y-down). Positive shoelace area = CCW
    // in Y-up math; reverse to make it CW.
    if compute_area(&centred) > 0.0 {
        centred.reverse();
    }

    let faults: Vec<f32> = (0..fault_count)
        .map(|_| rng.next_f32() * 2.0 * std::f32::consts::PI)
        .collect();
    (centred, faults)
}

/// Valtr's algorithm: builds `n` edge vectors whose x/y components each sum to
/// zero (so the chain closes), sorts them by angle, connects head-to-tail.
/// Sorting by angle guarantees convexity. Output is roughly unit-scale around
/// the origin. Port of `PolygonUtils.RandomConvexPolygon`.
fn random_convex_polygon(n: usize, rng: &mut Rng) -> Vec<Vec2> {
    let mut xs: Vec<f32> = (0..n).map(|_| rng.next_f32()).collect();
    let mut ys: Vec<f32> = (0..n).map(|_| rng.next_f32()).collect();
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let (min_x, max_x) = (xs[0], xs[n - 1]);
    let (min_y, max_y) = (ys[0], ys[n - 1]);

    let mut x_vec = vec![0.0f32; n];
    let mut y_vec = vec![0.0f32; n];

    let (mut last_top, mut last_bot) = (min_x, min_x);
    for i in 1..n - 1 {
        let x = xs[i];
        if rng.next_bool() {
            x_vec[i - 1] = x - last_top;
            last_top = x;
        } else {
            x_vec[i - 1] = last_bot - x;
            last_bot = x;
        }
    }
    x_vec[n - 2] = max_x - last_top;
    x_vec[n - 1] = last_bot - max_x;

    let (mut last_left, mut last_right) = (min_y, min_y);
    for i in 1..n - 1 {
        let y = ys[i];
        if rng.next_bool() {
            y_vec[i - 1] = y - last_left;
            last_left = y;
        } else {
            y_vec[i - 1] = last_right - y;
            last_right = y;
        }
    }
    y_vec[n - 2] = max_y - last_left;
    y_vec[n - 1] = last_right - max_y;

    // Randomly pair x- and y-components (Fisher-Yates shuffle of y), forming edge vectors.
    for i in (1..n).rev() {
        let j = rng.next_range(i + 1);
        y_vec.swap(i, j);
    }

    let mut vecs: Vec<Vec2> = (0..n).map(|i| Vec2::new(x_vec[i], y_vec[i])).collect();
    vecs.sort_by(|a, b| a.y.atan2(a.x).partial_cmp(&b.y.atan2(b.x)).unwrap());

    let mut pts = Vec::with_capacity(n);
    let mut cur = Vec2::ZERO;
    for v in vecs {
        pts.push(cur);
        cur += v;
    }
    pts
}

// ---------------------------------------------------------------------------
// Seeds + Voronoi cells.
// ---------------------------------------------------------------------------

fn scatter_seeds(bound: &[Vec2], grain_area: f32, rng: &mut Rng) -> Vec<Vec2> {
    let step = grain_area.max(1.0).sqrt();
    let mut min = Vec2::splat(f32::MAX);
    let mut max = Vec2::splat(f32::MIN);
    for &p in bound {
        min = min.min(p);
        max = max.max(p);
    }

    let mut seeds = Vec::new();
    let mut y = min.y;
    while y < max.y {
        let mut x = min.x;
        while x < max.x {
            let p = Vec2::new(
                x + (rng.next_f32() - 0.5) * step * 0.7,
                y + (rng.next_f32() - 0.5) * step * 0.7,
            );
            if contains_point(bound, p) {
                seeds.push(p);
            }
            x += step;
        }
        y += step;
    }

    while seeds.len() < 3 {
        seeds.push(Vec2::new(
            (min.x + max.x) * 0.5 + (rng.next_f32() - 0.5) * step,
            (min.y + max.y) * 0.5 + (rng.next_f32() - 0.5) * step,
        ));
    }
    seeds
}

/// Lloyd relaxation: each pass moves every seed to its (bound-clipped) Voronoi
/// cell's centroid, converging toward a centroidal Voronoi tessellation.
fn lloyd_relax(seeds: &mut [Vec2], bound: &[Vec2], iterations: usize) {
    if seeds.len() < 2 {
        return;
    }
    let mut next = vec![Vec2::ZERO; seeds.len()];
    for _ in 0..iterations {
        for i in 0..seeds.len() {
            let poly = voronoi_cell(i, seeds, bound);
            next[i] = if poly.len() >= 3 && compute_area(&poly).abs() > MIN_CELL_AREA {
                compute_centroid(&poly)
            } else {
                seeds[i]
            };
        }
        seeds.copy_from_slice(&next);
    }
}

/// Voronoi cell of `seeds[self_idx]`: the bound clipped by the perpendicular
/// bisector half-plane (keeping the seed's side) against every other seed.
fn voronoi_cell(self_idx: usize, seeds: &[Vec2], bound: &[Vec2]) -> Vec<Vec2> {
    let s = seeds[self_idx];
    let mut poly: Vec<Vec2> = bound.to_vec();
    for (j, &t) in seeds.iter().enumerate() {
        if j == self_idx {
            continue;
        }
        let n_raw = s - t;
        let len = n_raw.length();
        if len < 1e-6 {
            continue;
        }
        let n = n_raw / len;
        let mid = (s + t) * 0.5;
        poly = clip_convex_by_half_plane(&poly, mid, n);
        if poly.len() < 3 {
            break;
        }
    }
    poly
}

fn shared_edge_length(a: &[Vec2], b: &[Vec2]) -> f32 {
    let mut shared = 0.0f32;
    for i in 0..a.len() {
        let (a0, a1) = (a[i], a[(i + 1) % a.len()]);
        for j in 0..b.len() {
            let (b0, b1) = (b[j], b[(j + 1) % b.len()]);
            shared += segment_overlap(a0, a1, b0, b1);
        }
    }
    shared
}

/// Length of the collinear overlap between segments a0-a1 and b0-b1 (0 if not
/// collinear within tolerance). Voronoi-shared edges lie on the same bisector
/// line, so their overlap is the bond's edge length.
fn segment_overlap(a0: Vec2, a1: Vec2, b0: Vec2, b1: Vec2) -> f32 {
    let ab = a1 - a0;
    let l = ab.length();
    if l < 1e-6 {
        return 0.0;
    }
    let u = ab / l;
    let perp = Vec2::new(-u.y, u.x);
    if (b0 - a0).dot(perp).abs() > COLLINEAR_TOL {
        return 0.0;
    }
    if (b1 - a0).dot(perp).abs() > COLLINEAR_TOL {
        return 0.0;
    }
    let tb0 = (b0 - a0).dot(u);
    let tb1 = (b1 - a0).dot(u);
    let lo = tb0.min(tb1).max(0.0);
    let hi = tb0.max(tb1).min(l);
    (hi - lo).max(0.0)
}

fn build_bonds(cells: &[Cell], toughness: f32) -> Vec<Bond> {
    let mut bonds = Vec::new();
    for i in 0..cells.len() {
        for j in i + 1..cells.len() {
            let shared = shared_edge_length(&cells[i].local, &cells[j].local);
            if shared > MIN_SHARED_EDGE {
                bonds.push(Bond {
                    a: i,
                    b: j,
                    edge_length: shared,
                    strength: shared * toughness,
                    strength_mult: 1.0,
                    stress: 0.0,
                    broken: false,
                });
            }
        }
    }
    bonds
}

fn build_bonds_weighted(
    cells: &[Cell],
    seed_indices: &[usize],
    mults: &[f32],
    toughness: f32,
) -> Vec<Bond> {
    let mult_of = |seed_idx: usize| mults.get(seed_idx).copied().unwrap_or(1.0);
    let mut bonds = Vec::new();
    for i in 0..cells.len() {
        for j in i + 1..cells.len() {
            let shared = shared_edge_length(&cells[i].local, &cells[j].local);
            if shared > MIN_SHARED_EDGE {
                let bm = (mult_of(seed_indices[i]) * mult_of(seed_indices[j])).sqrt();
                bonds.push(Bond {
                    a: i,
                    b: j,
                    edge_length: shared,
                    strength: shared * toughness * bm,
                    strength_mult: bm,
                    stress: 0.0,
                    broken: false,
                });
            }
        }
    }
    bonds
}

fn cells_from_polys(polys: &[Vec<Vec2>]) -> (Vec2, Vec<Cell>) {
    let mut total_area = 0.0f32;
    let mut body_centroid = Vec2::ZERO;
    for poly in polys {
        let a = compute_area(poly).abs();
        body_centroid += compute_centroid(poly) * a;
        total_area += a;
    }
    body_centroid /= total_area;

    let cells = polys
        .iter()
        .map(|world| {
            let local: Vec<Vec2> = world.iter().map(|&v| v - body_centroid).collect();
            Cell {
                area: compute_area(world).abs(),
                centroid: compute_centroid(world) - body_centroid,
                local,
                ..Cell::default()
            }
        })
        .collect();
    (body_centroid, cells)
}

// ---------------------------------------------------------------------------
// Public builders.
// ---------------------------------------------------------------------------

/// Builds a fracturable asteroid: a random convex outline of the given radius,
/// tessellated by the material grain. `membership` may carve concavities
/// (return false to drop a cell); `None` = solid convex blob. Port of `BuildAsteroid`.
pub fn build_asteroid(
    sides: usize,
    radius: f32,
    material: FractureProperties,
    membership: Option<&dyn Fn(Vec2) -> bool>,
    rng: &mut Rng,
) -> FracturableBody {
    let (bound, _) = generate_convex(sides, radius, rng, 3);
    build(&bound, material, membership, rng)
}

/// Builds a fracturable body from an explicit convex bound. Port of `Build`.
pub fn build(
    convex_bound: &[Vec2],
    material: FractureProperties,
    membership: Option<&dyn Fn(Vec2) -> bool>,
    rng: &mut Rng,
) -> FracturableBody {
    let seeds = scatter_seeds(convex_bound, material.grain_area, rng);

    let mut kept: Vec<Vec<Vec2>> = Vec::with_capacity(seeds.len());
    for (i, &seed) in seeds.iter().enumerate() {
        let poly = voronoi_cell(i, &seeds, convex_bound);
        if poly.len() < 3 || compute_area(&poly).abs() < MIN_CELL_AREA {
            continue;
        }
        if let Some(m) = membership {
            if !m(seed) {
                continue;
            }
        }
        kept.push(poly);
    }
    if kept.is_empty() {
        kept.push(convex_bound.to_vec());
    }

    let (_, cells) = cells_from_polys(&kept);
    let bonds = build_bonds(&cells, material.toughness);

    FracturableBody {
        cells,
        bonds,
        material,
        state: FractureState {
            rng_seed: rng.next_u32(),
        },
        fragile: false,
    }
}

/// Builds a fracturable body from explicit seed positions, with per-seed bond
/// multipliers and an optional membership predicate for carving concavities.
/// The full Voronoi is computed over ALL seeds; cells whose seed fails
/// membership are dropped, then the largest connected surviving component is
/// kept and enclosed holes are filled back in. Port of `BuildWithSeeds`.
#[allow(clippy::too_many_arguments)]
pub fn build_with_seeds(
    convex_bound: &[Vec2],
    seed_positions: &[Vec2],
    seed_bond_mults: &[f32],
    membership: Option<&dyn Fn(Vec2) -> bool>,
    material: FractureProperties,
    rng: &mut Rng,
    relax_iterations: usize,
) -> FracturableBody {
    assert!(convex_bound.len() >= 3, "bound needs >= 3 vertices");
    assert!(!seed_positions.is_empty(), "at least one seed required");

    let mut seeds = seed_positions.to_vec();
    if relax_iterations > 0 {
        lloyd_relax(&mut seeds, convex_bound, relax_iterations);
    }

    // 1. Full Voronoi over ALL seeds → every area-valid cell, recording its seed
    //    index, whether it passed membership, and whether it touches the bound.
    let mut polys: Vec<Vec<Vec2>> = Vec::with_capacity(seeds.len());
    let mut sidx: Vec<usize> = Vec::with_capacity(seeds.len());
    let mut pass: Vec<bool> = Vec::with_capacity(seeds.len());
    let mut hull: Vec<bool> = Vec::with_capacity(seeds.len());
    for (i, &seed) in seeds.iter().enumerate() {
        let poly = voronoi_cell(i, &seeds, convex_bound);
        if poly.len() < 3 || compute_area(&poly).abs() < MIN_CELL_AREA {
            continue;
        }
        let passes = membership.map(|m| m(seed)).unwrap_or(true);
        let touches_bound = shared_edge_length(&poly, convex_bound) > MIN_SHARED_EDGE;
        polys.push(poly);
        sidx.push(i);
        pass.push(passes);
        hull.push(touches_bound);
    }
    let m = polys.len();

    // 2. Adjacency over all valid cells (shared Voronoi edges).
    let mut nbr: Vec<Vec<usize>> = vec![Vec::new(); m];
    let mut nbr_len: Vec<Vec<f32>> = vec![Vec::new(); m];
    for i in 0..m {
        for j in i + 1..m {
            let shared = shared_edge_length(&polys[i], &polys[j]);
            if shared > MIN_SHARED_EDGE {
                nbr[i].push(j);
                nbr_len[i].push(shared);
                nbr[j].push(i);
                nbr_len[j].push(shared);
            }
        }
    }

    // 3. Keep only the largest connected component of membership-passed cells.
    let mut keep = vec![false; m];
    let mut comp = vec![-1i32; m];
    let mut best_comp = -1i32;
    let mut best_size = 0usize;
    let mut cid = 0i32;
    for s in 0..m {
        if !pass[s] || comp[s] != -1 {
            continue;
        }
        let mut size = 0usize;
        comp[s] = cid;
        let mut stack = vec![s];
        while let Some(u) = stack.pop() {
            size += 1;
            for &v in &nbr[u] {
                if pass[v] && comp[v] == -1 {
                    comp[v] = cid;
                    stack.push(v);
                }
            }
        }
        if size > best_size {
            best_size = size;
            best_comp = cid;
        }
        cid += 1;
    }
    for i in 0..m {
        if comp[i] == best_comp {
            keep[i] = true;
        }
    }

    // 4. Hole-fill: any empty region that never touches a hull cell is an
    //    enclosed void → fill it back. Boundary concavities stay open.
    let mut seen = vec![false; m];
    for s in 0..m {
        if keep[s] || seen[s] {
            continue;
        }
        let mut region = Vec::new();
        let mut touches_hull = false;
        seen[s] = true;
        let mut stack = vec![s];
        while let Some(u) = stack.pop() {
            region.push(u);
            if hull[u] {
                touches_hull = true;
            }
            for &v in &nbr[u] {
                if !keep[v] && !seen[v] {
                    seen[v] = true;
                    stack.push(v);
                }
            }
        }
        if !touches_hull {
            for c in region {
                keep[c] = true;
            }
        }
    }

    // 5. Materialize kept cells + bonds.
    let mut local_of = vec![-1i32; m];
    let mut kept_polys: Vec<Vec<Vec2>> = Vec::new();
    for i in 0..m {
        if keep[i] {
            local_of[i] = kept_polys.len() as i32;
            kept_polys.push(polys[i].clone());
        }
    }
    if kept_polys.is_empty() {
        kept_polys.push(convex_bound.to_vec());
    }

    let (_, cells) = cells_from_polys(&kept_polys);

    let mut bonds = Vec::new();
    for i in 0..m {
        if local_of[i] < 0 {
            continue;
        }
        for (t, &j) in nbr[i].iter().enumerate() {
            if j <= i || local_of[j] < 0 {
                continue; // each kept↔kept pair once
            }
            let mi = seed_bond_mults.get(sidx[i]).copied().unwrap_or(1.0);
            let mj = seed_bond_mults.get(sidx[j]).copied().unwrap_or(1.0);
            let bm = (mi * mj).sqrt();
            bonds.push(Bond {
                a: local_of[i] as usize,
                b: local_of[j] as usize,
                edge_length: nbr_len[i][t],
                strength: nbr_len[i][t] * material.toughness * bm,
                strength_mult: bm,
                stress: 0.0,
                broken: false,
            });
        }
    }

    FracturableBody {
        cells,
        bonds,
        material,
        state: FractureState {
            rng_seed: rng.next_u32(),
        },
        fragile: false,
    }
}

/// Builds a fracturable body from an authored shape: explicit seed positions
/// tessellate the outline directly (not its convex hull, so non-convex
/// silhouettes like ship wings are preserved). Port of `BuildFromExplicitSeeds`.
pub fn build_from_explicit_seeds(
    outline: &[Vec2],
    seed_positions: &[Vec2],
    seed_bond_mults: &[f32],
    material: FractureProperties,
    rng: &mut Rng,
) -> FracturableBody {
    assert!(outline.len() >= 3, "outline needs >= 3 vertices");
    assert!(!seed_positions.is_empty(), "at least one seed required");

    let mut kept: Vec<Vec<Vec2>> = Vec::with_capacity(seed_positions.len());
    let mut kept_idx: Vec<usize> = Vec::with_capacity(seed_positions.len());
    for (i, _) in seed_positions.iter().enumerate() {
        let poly = voronoi_cell(i, seed_positions, outline);
        if poly.len() < 3 || compute_area(&poly).abs() < MIN_CELL_AREA {
            continue;
        }
        kept.push(poly);
        kept_idx.push(i);
    }
    if kept.is_empty() {
        kept.push(outline.to_vec());
        kept_idx.push(0);
    }

    let (_, cells) = cells_from_polys(&kept);
    let bonds = build_bonds_weighted(&cells, &kept_idx, seed_bond_mults, material.toughness);

    FracturableBody {
        cells,
        bonds,
        material,
        state: FractureState {
            rng_seed: rng.next_u32(),
        },
        fragile: false,
    }
}
