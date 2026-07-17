//! Golden tests for the ported fracture core. These lock down the invariants
//! that matter for a faithful port: the energy model, moment of inertia, exact
//! area conservation across a split, cascading shatter, pulverisation, and
//! determinism. No Bevy, no renderer — the payoff of a pure crate.

use fracture::*;
use glam::Vec2;

// --- builders ---------------------------------------------------------------

fn mat() -> FractureProperties {
    FractureProperties {
        toughness: 1.0,
        restitution: 0.0,
        relax_rate: 0.0,
        brittleness: 0.9,
        crack_speed: 10.0,
        grain_area: 100.0,
        min_fragment_area: 20.0,
        density: 1.0,
        cell_toughness: 1.0,
        spin_pre_stress: 0.0,
        crack_directionality: 0.0,
        detach_cell_scale: 0.0, // 0 → no rng jitter, fully deterministic geometry
        detach_cell_jitter: 0.0,
    }
}

/// An nx×ny grid of s×s square cells in body-local space (centred on the origin),
/// with 4-neighbour bonds of the given strength. cell index = gy*nx + gx.
fn grid(nx: usize, ny: usize, s: f32, strength: f32, cell_toughness: f32) -> FracturableBody {
    let mut cells = Vec::new();
    for gy in 0..ny {
        for gx in 0..nx {
            let cx = (gx as f32 - (nx as f32 - 1.0) / 2.0) * s;
            let cy = (gy as f32 - (ny as f32 - 1.0) / 2.0) * s;
            let h = s / 2.0;
            cells.push(Cell {
                local: vec![
                    Vec2::new(cx - h, cy - h),
                    Vec2::new(cx + h, cy - h),
                    Vec2::new(cx + h, cy + h),
                    Vec2::new(cx - h, cy + h),
                ],
                centroid: Vec2::new(cx, cy),
                area: s * s,
                ..Cell::default()
            });
        }
    }
    let idx = |gx: usize, gy: usize| gy * nx + gx;
    let mut bonds = Vec::new();
    let mut push = |a: usize, b: usize| {
        bonds.push(Bond {
            a,
            b,
            edge_length: s,
            strength,
            strength_mult: 1.0,
            stress: 0.0,
            broken: false,
        })
    };
    for gy in 0..ny {
        for gx in 0..nx {
            if gx + 1 < nx {
                push(idx(gx, gy), idx(gx + 1, gy));
            }
            if gy + 1 < ny {
                push(idx(gx, gy), idx(gx, gy + 1));
            }
        }
    }
    let mut m = mat();
    m.cell_toughness = cell_toughness;
    FracturableBody {
        cells,
        bonds,
        material: m,
        state: FractureState::default(),
        fragile: false,
    }
}

fn input_for(body: &FracturableBody, impact: Vec2) -> FractureInput {
    FractureInput {
        impact_point_world: impact,
        impact_dir: Vec2::X,
        directionality: 0.0,
        blast_fraction: 0.0,
        body_position: Vec2::ZERO,
        body_rotation: 0.0,
        body_linear: Vec2::ZERO,
        body_angular: 0.0,
        body_mass: total_area(body), // mass == area ⇒ fragment mass == fragment area
    }
}

fn total_area(body: &FracturableBody) -> f32 {
    body.cells.iter().map(|c| c.area).sum()
}

// --- geometry / energy ------------------------------------------------------

#[test]
fn inertia_of_a_square_matches_the_closed_form() {
    // Square of side 2 about its centroid: I = m·(w²+h²)/12 = m·8/12 = 2m/3.
    let verts = [
        Vec2::new(-1.0, -1.0),
        Vec2::new(1.0, -1.0),
        Vec2::new(1.0, 1.0),
        Vec2::new(-1.0, 1.0),
    ];
    let i = compute_inertia(&verts, 3.0);
    assert!((i - 2.0).abs() < 1e-4, "expected 2.0, got {i}");
}

#[test]
fn compute_energy_matches_a_hand_computed_value() {
    // impactor_mass 0 ⇒ invMass = 1/m_body; no rotation. m_eff = m_body = 2.
    // E = ENERGY_SCALE·½·2·100² · (1-0) = 0.0001·10000 = 1.0.
    let e = compute_energy(Vec2::ZERO, Vec2::X, 100.0, 0.0, Vec2::ZERO, 2.0, 0.0, 0.0);
    assert!((e - 1.0).abs() < 1e-5, "expected 1.0, got {e}");
}

#[test]
fn effective_directionality_is_the_mean() {
    assert!((effective_directionality(0.4, 0.8) - 0.6).abs() < 1e-6);
}

#[test]
fn spin_mul_is_unity_without_spin() {
    let body = grid(3, 3, 10.0, 1.0, 1.0);
    let (spin_mul, adj) = prepare_graph(&body, 0.0);
    assert_eq!(spin_mul.len(), body.bonds.len());
    assert!(spin_mul.iter().all(|&m| (m - 1.0).abs() < 1e-9));
    assert_eq!(adj.len(), body.cells.len());
}

// --- fragment construction --------------------------------------------------

#[test]
fn a_prebroken_body_splits_and_conserves_area_and_mass() {
    // 1×3 chain; break the middle bond ⇒ {0,1} and {2}.
    let body = grid(3, 1, 10.0, 1.0, 1.0);
    let input = input_for(&body, Vec2::new(-10.0, 0.0));
    let broken = [false, true]; // bonds: (0-1), (1-2)
    let pulverized = [false, false, false];
    let mut rng = Rng::new(1);

    let frags = build_result(
        &body,
        &input,
        &broken,
        &pulverized,
        &[0.0; 3],
        &mut rng,
        true,
    );
    assert_eq!(frags.len(), 2, "one broken bond in a chain ⇒ two fragments");

    let frag_area: f32 = frags.iter().map(|f| f.area).sum();
    assert!(
        (frag_area - total_area(&body)).abs() < 1e-3,
        "area must be conserved"
    );

    // mass == area here, so the two-cell piece is twice the one-cell piece.
    let mut areas: Vec<f32> = frags.iter().map(|f| f.area).collect();
    areas.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert!((areas[0] - 100.0).abs() < 1e-3 && (areas[1] - 200.0).abs() < 1e-3);
    for f in &frags {
        assert!((f.mass - f.area).abs() < 1e-3, "mass tracks area share");
    }
}

// --- full pipeline ----------------------------------------------------------

#[test]
fn a_hard_hit_cascades_into_multiple_fragments() {
    let body_ref = grid(4, 4, 10.0, 1.0, 1_000.0); // tough cells (no vaporise), weak bonds
    let mut body = body_ref.clone();
    let n = body.cells.len();

    let mut proc = seed_process(
        &body,
        0,                       // struck the corner cell
        Vec2::new(-15.0, -15.0), // impact point (world == local here)
        Vec2::ZERO,              // body position
        0.0,                     // rotation
        0.0,                     // angular
        Vec2::new(1.0, 1.0),     // dir into the body
        2000.0,                  // energy — plenty to shatter
        &WeaponProfile {
            directionality: 0.0,
            blast_fraction: 0.0,
            knockback: 0.0,
        },
        0.0,
    );

    let pulv = drive_to_completion(&mut body, &mut proc);
    assert!(
        pulv.is_empty(),
        "cell_toughness is huge ⇒ nothing vaporises"
    );

    let comps = count_components(n, &body.bonds, &proc.broken, &proc.pulverized);
    assert!(
        comps > 1,
        "a hard hit on weak bonds must split the body (got {comps})"
    );
    assert!(
        proc.broken.iter().any(|&b| b),
        "some bonds must have broken"
    );

    // Finalise and confirm exact area conservation (nothing pulverised).
    let input = input_for(&body_ref, Vec2::new(-15.0, -15.0));
    let mut rng = Rng::new(7);
    let frags = build_result(
        &body,
        &input,
        &proc.broken,
        &proc.pulverized,
        &proc.fling_e,
        &mut rng,
        true,
    );
    assert_eq!(frags.len(), comps, "one fragment per surviving component");
    let frag_area: f32 = frags.iter().map(|f| f.area).sum();
    assert!(
        (frag_area - total_area(&body_ref)).abs() < 1e-2,
        "area conserved across the shatter"
    );
}

#[test]
fn full_blast_on_a_lone_cell_pulverises_it() {
    // Single cell, no bonds, blast=1, weak cell ⇒ vaporises whole (0 fragments).
    let mut body = grid(1, 1, 10.0, 1.0, 0.01);
    let mut proc = seed_process(
        &body,
        0,
        Vec2::ZERO,
        Vec2::ZERO,
        0.0,
        0.0,
        Vec2::X,
        5000.0,
        &WeaponProfile {
            directionality: 0.0,
            blast_fraction: 1.0,
            knockback: 0.0,
        },
        0.0,
    );
    let pulv = drive_to_completion(&mut body, &mut proc);
    assert_eq!(pulv, vec![0], "the only cell should pulverise");
    assert!(proc.pulverized[0]);

    let input = input_for(&body, Vec2::ZERO);
    let mut rng = Rng::new(1);
    let frags = build_result(
        &body,
        &input,
        &proc.broken,
        &proc.pulverized,
        &proc.fling_e,
        &mut rng,
        true,
    );
    assert!(
        frags.is_empty(),
        "a fully vaporised body yields no fragment bodies"
    );
}

#[test]
fn area_is_conserved_even_with_partial_pulverisation() {
    let body_ref = grid(3, 3, 10.0, 1.0, 0.5); // some cells will vaporise
    let mut body = body_ref.clone();
    let mut proc = seed_process(
        &body,
        4,
        Vec2::ZERO,
        Vec2::ZERO,
        0.0,
        0.0,
        Vec2::X,
        3000.0,
        &WeaponProfile {
            directionality: 0.0,
            blast_fraction: 0.6,
            knockback: 0.0,
        },
        0.0,
    );
    drive_to_completion(&mut body, &mut proc);

    let pulverised_area: f32 = (0..body.cells.len())
        .filter(|&i| proc.pulverized[i])
        .map(|i| body.cells[i].area)
        .sum();

    let input = input_for(&body_ref, Vec2::ZERO);
    let mut rng = Rng::new(3);
    let frags = build_result(
        &body,
        &input,
        &proc.broken,
        &proc.pulverized,
        &proc.fling_e,
        &mut rng,
        true,
    );
    let frag_area: f32 = frags.iter().map(|f| f.area).sum();

    assert!(
        (frag_area + pulverised_area - total_area(&body_ref)).abs() < 1e-2,
        "surviving + pulverised area must equal the original ({frag_area} + {pulverised_area} vs {})",
        total_area(&body_ref)
    );
}

#[test]
fn the_same_seed_produces_identical_fractures() {
    let run = || {
        let mut body = grid(4, 4, 10.0, 1.0, 2.0);
        let mut proc = seed_process(
            &body,
            0,
            Vec2::new(-15.0, -15.0),
            Vec2::ZERO,
            0.0,
            0.0,
            Vec2::new(1.0, 1.0),
            2500.0,
            &WeaponProfile {
                directionality: 0.3,
                blast_fraction: 0.4,
                knockback: 0.0,
            },
            400.0,
        );
        drive_to_completion(&mut body, &mut proc);
        let input = input_for(&grid(4, 4, 10.0, 1.0, 2.0), Vec2::new(-15.0, -15.0));
        let mut rng = Rng::new(42);
        build_result(
            &body,
            &input,
            &proc.broken,
            &proc.pulverized,
            &proc.fling_e,
            &mut rng,
            true,
        )
    };
    let a = run();
    let b = run();
    assert_eq!(a.len(), b.len(), "fragment count must be reproducible");
    for (fa, fb) in a.iter().zip(&b) {
        assert!((fa.area - fb.area).abs() < 1e-4);
        assert!(
            (fa.linear - fb.linear).length() < 1e-3,
            "derived motion must be reproducible"
        );
        assert!((fa.angular - fb.angular).abs() < 1e-4);
    }
}
