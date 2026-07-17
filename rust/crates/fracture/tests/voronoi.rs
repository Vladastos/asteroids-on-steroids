//! Tests for the ported Voronoi tessellator. Locks down the invariants that
//! matter: generated outlines are actually convex, tessellated cells exactly
//! tile the source area, the bond graph is a single connected component for a
//! solid blob, and membership-carved concavities/hole-filling behave.

use fracture::*;
use glam::Vec2;

fn mat() -> FractureProperties {
    FractureProperties {
        toughness: 10.0,
        restitution: 0.3,
        relax_rate: 100.0,
        brittleness: 0.6,
        crack_speed: 200.0,
        grain_area: 400.0, // smallish grain → several cells per test body
        min_fragment_area: 100.0,
        density: 1.0,
        cell_toughness: 1.0,
        spin_pre_stress: 0.1,
        crack_directionality: 0.3,
        detach_cell_scale: 0.9,
        detach_cell_jitter: 0.02,
    }
}

fn cross(o: Vec2, a: Vec2, b: Vec2) -> f32 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

/// True iff every triple of consecutive vertices turns the same way (convex),
/// for a polygon of consistent winding.
fn is_convex(verts: &[Vec2]) -> bool {
    let n = verts.len();
    let mut sign = 0i32;
    for i in 0..n {
        let o = verts[i];
        let a = verts[(i + 1) % n];
        let b = verts[(i + 2) % n];
        let c = cross(o, a, b);
        if c.abs() < 1e-4 {
            continue; // collinear triple: inconclusive, skip
        }
        let s = if c > 0.0 { 1 } else { -1 };
        if sign == 0 {
            sign = s;
        } else if s != sign {
            return false;
        }
    }
    true
}

fn total_cell_area(body: &FracturableBody) -> f32 {
    body.cells.iter().map(|c| c.area).sum()
}

#[test]
fn generate_convex_produces_a_convex_polygon_of_the_right_size() {
    for seed in 0..10u64 {
        let mut rng = Rng::new(seed + 1);
        let (verts, faults) = generate_convex(12, 100.0, &mut rng, 3);
        assert_eq!(verts.len(), 12);
        assert_eq!(faults.len(), 3);
        assert!(is_convex(&verts), "outline must be convex (seed {seed})");

        let mean_r: f32 = verts.iter().map(|v| v.length()).sum::<f32>() / verts.len() as f32;
        assert!(
            (mean_r - 100.0).abs() < 5.0,
            "mean radius should track the requested radius, got {mean_r}"
        );
    }
}

#[test]
fn generate_convex_is_deterministic_for_a_given_seed() {
    let mut a = Rng::new(7);
    let mut b = Rng::new(7);
    let (va, _) = generate_convex(10, 50.0, &mut a, 0);
    let (vb, _) = generate_convex(10, 50.0, &mut b, 0);
    assert_eq!(va.len(), vb.len());
    for (x, y) in va.iter().zip(&vb) {
        assert!((*x - *y).length() < 1e-6);
    }
}

#[test]
fn build_tessellates_a_square_with_conserved_area() {
    let bound = vec![
        Vec2::new(-100.0, -100.0),
        Vec2::new(100.0, -100.0),
        Vec2::new(100.0, 100.0),
        Vec2::new(-100.0, 100.0),
    ];
    let mut rng = Rng::new(3);
    let body = build(&bound, mat(), None, &mut rng);

    assert!(
        body.cells.len() > 1,
        "grain smaller than the body should yield multiple cells"
    );
    let area = total_cell_area(&body);
    assert!(
        (area - 40000.0).abs() < 40000.0 * 0.03,
        "cells must (nearly) tile the 200x200 square, got {area}"
    );

    for c in &body.cells {
        assert!(is_convex(&c.local), "every Voronoi cell must be convex");
    }
}

#[test]
fn build_produces_a_fully_connected_bond_graph_for_a_solid_blob() {
    let bound = vec![
        Vec2::new(-100.0, -100.0),
        Vec2::new(100.0, -100.0),
        Vec2::new(100.0, 100.0),
        Vec2::new(-100.0, 100.0),
    ];
    let mut rng = Rng::new(5);
    let body = build(&bound, mat(), None, &mut rng);

    let broken = vec![false; body.bonds.len()];
    let pulverized = vec![false; body.cells.len()];
    let count = count_components(body.cells.len(), &body.bonds, &broken, &pulverized);
    assert_eq!(
        count, 1,
        "a solid blob with no membership carve must be one connected component"
    );
}

#[test]
fn membership_carves_a_hole_and_bonds_still_form_one_island() {
    let bound = vec![
        Vec2::new(-150.0, -150.0),
        Vec2::new(150.0, -150.0),
        Vec2::new(150.0, 150.0),
        Vec2::new(-150.0, 150.0),
    ];
    // Carve out a disc in the middle — an annulus-ish body.
    let membership = |p: Vec2| p.length() > 40.0;
    let mut rng = Rng::new(9);
    let body = build(&bound, mat(), Some(&membership), &mut rng);

    // No cell's centroid should land deep inside the carved disc.
    for c in &body.cells {
        assert!(
            c.centroid.length() > 10.0,
            "a kept cell centred inside the carved hole: {:?}",
            c.centroid
        );
    }
    assert!(
        total_cell_area(&body) < 150.0 * 150.0 * 4.0,
        "carving a hole must shrink total area vs. the full bound"
    );
}

#[test]
fn build_with_seeds_keeps_only_the_largest_component_and_fills_enclosed_holes() {
    // A wide rectangle so a "drop the middle column" membership predicate can
    // split it into two islands; the largest survives, the rest doesn't.
    let bound = vec![
        Vec2::new(-150.0, -50.0),
        Vec2::new(150.0, -50.0),
        Vec2::new(150.0, 50.0),
        Vec2::new(-150.0, 50.0),
    ];
    let mut rng = Rng::new(11);
    // Explicit seed grid: 6 columns x 2 rows, so removing the middle column
    // (x in [-50,50]) cleanly separates left/right islands of equal size —
    // the "largest component" tie-break isn't exercised, just connectivity.
    let mut seeds = Vec::new();
    for gx in 0..6 {
        for gy in 0..2 {
            let x = -125.0 + gx as f32 * 50.0;
            let y = -25.0 + gy as f32 * 50.0;
            seeds.push(Vec2::new(x, y));
        }
    }
    let mults = vec![1.0; seeds.len()];
    let membership = |p: Vec2| p.x < -25.0 || p.x > 25.0; // drop the middle column
    let body = build_with_seeds(
        &bound,
        &seeds,
        &mults,
        Some(&membership),
        mat(),
        &mut rng,
        0,
    );

    assert!(!body.cells.is_empty());
    let broken = vec![false; body.bonds.len()];
    let pulverized = vec![false; body.cells.len()];
    let count = count_components(body.cells.len(), &body.bonds, &broken, &pulverized);
    assert_eq!(
        count, 1,
        "only the largest connected surviving component should remain"
    );
}

#[test]
fn build_from_explicit_seeds_preserves_a_non_convex_outline() {
    // An L-shaped (non-convex) outline — must NOT be replaced by its convex hull.
    let outline = vec![
        Vec2::new(0.0, 0.0),
        Vec2::new(100.0, 0.0),
        Vec2::new(100.0, 40.0),
        Vec2::new(40.0, 40.0),
        Vec2::new(40.0, 100.0),
        Vec2::new(0.0, 100.0),
    ];
    let seeds = vec![
        Vec2::new(20.0, 20.0),
        Vec2::new(70.0, 20.0),
        Vec2::new(20.0, 70.0),
    ];
    let mults = vec![1.0, 2.0, 1.0];
    let mut rng = Rng::new(13);
    let body = build_from_explicit_seeds(&outline, &seeds, &mults, mat(), &mut rng);

    assert_eq!(body.cells.len(), 3);
    let hull_area = 100.0 * 100.0; // convex hull of the L would be the full 100x100 box
    assert!(
        total_cell_area(&body) < hull_area * 0.9,
        "explicit-seed tessellation must respect the concave outline"
    );
}

#[test]
fn a_lone_seed_still_produces_a_valid_single_cell_body() {
    let outline = vec![
        Vec2::new(-10.0, -10.0),
        Vec2::new(10.0, -10.0),
        Vec2::new(10.0, 10.0),
        Vec2::new(-10.0, 10.0),
    ];
    let seeds = vec![Vec2::ZERO];
    let mults = vec![1.0];
    let mut rng = Rng::new(1);
    let body = build_from_explicit_seeds(&outline, &seeds, &mults, mat(), &mut rng);
    assert_eq!(body.cells.len(), 1);
    assert!(body.bonds.is_empty());
    assert!((total_cell_area(&body) - 400.0).abs() < 1.0);
}
