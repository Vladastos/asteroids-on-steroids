//! Tests for the ported shape/contact math. These lock down the normal
//! convention ("points from B into A") across every shape pair, which is the
//! part most at risk of transcription error from the C#'s double-dispatch code.

use collision::*;
use glam::Vec2;

fn square(half: f32) -> Shape {
    Shape::Polygon(Polygon::new(vec![
        Vec2::new(-half, -half),
        Vec2::new(half, -half),
        Vec2::new(half, half),
        Vec2::new(-half, half),
    ]))
}

/// After a contact, moving A along `normal * depth` should just separate it.
fn assert_separates(pos_a: Vec2, rot_a: f32, a: &Shape, pos_b: Vec2, rot_b: f32, b: &Shape) {
    let c = intersects(pos_a, rot_a, a, pos_b, rot_b, b).expect("expected a contact");
    let moved = pos_a + c.normal * (c.depth + 1e-3);
    assert!(
        intersects(moved, rot_a, a, pos_b, rot_b, b).is_none(),
        "moving A by normal*depth should separate the shapes (normal={:?} depth={})",
        c.normal,
        c.depth
    );
}

#[test]
fn circle_vs_circle_separates_along_normal() {
    let a = Shape::Circle { radius: 10.0 };
    let b = Shape::Circle { radius: 10.0 };
    assert_separates(Vec2::new(5.0, 0.0), 0.0, &a, Vec2::ZERO, 0.0, &b);
}

#[test]
fn circle_vs_circle_normal_points_toward_a() {
    let a = Shape::Circle { radius: 5.0 };
    let b = Shape::Circle { radius: 5.0 };
    // A is to the +X side of B ⇒ normal (B→A) should point +X.
    let c = intersects(Vec2::new(3.0, 0.0), 0.0, &a, Vec2::ZERO, 0.0, &b).unwrap();
    assert!(c.normal.x > 0.9, "expected normal ≈ +X, got {:?}", c.normal);
}

#[test]
fn circle_vs_polygon_separates_both_ways() {
    let circle = Shape::Circle { radius: 8.0 };
    let poly = square(10.0);
    assert_separates(Vec2::new(15.0, 0.0), 0.0, &circle, Vec2::ZERO, 0.0, &poly);
    assert_separates(Vec2::ZERO, 0.0, &poly, Vec2::new(15.0, 0.0), 0.0, &circle);
}

#[test]
fn polygon_vs_polygon_separates_along_normal() {
    let a = square(10.0);
    let b = square(10.0);
    assert_separates(Vec2::new(15.0, 0.0), 0.0, &a, Vec2::ZERO, 0.0, &b);
    assert_separates(Vec2::new(0.0, 15.0), 0.0, &a, Vec2::ZERO, 0.0, &b);
}

#[test]
fn circle_vs_aabb_separates_both_ways() {
    let circle = Shape::Circle { radius: 8.0 };
    let aabb = Shape::Aabb {
        half_width: 10.0,
        half_height: 10.0,
    };
    assert_separates(Vec2::new(15.0, 0.0), 0.0, &circle, Vec2::ZERO, 0.0, &aabb);
    assert_separates(Vec2::ZERO, 0.0, &aabb, Vec2::new(15.0, 0.0), 0.0, &circle);
}

#[test]
fn aabb_vs_aabb_separates_along_normal() {
    let a = Shape::Aabb {
        half_width: 10.0,
        half_height: 10.0,
    };
    let b = Shape::Aabb {
        half_width: 10.0,
        half_height: 10.0,
    };
    assert_separates(Vec2::new(15.0, 0.0), 0.0, &a, Vec2::ZERO, 0.0, &b);
}

#[test]
fn no_contact_when_far_apart() {
    let a = Shape::Circle { radius: 5.0 };
    let b = square(5.0);
    assert!(intersects(Vec2::new(1000.0, 0.0), 0.0, &a, Vec2::ZERO, 0.0, &b).is_none());
}

// --- compound ----------------------------------------------------------------

fn two_cell_compound() -> Compound {
    // Two unit squares side by side in compound-local space: [-10,0]x[-10,10] and
    // [0,10]x[-10,10].
    let left = Shape::Polygon(Polygon::new(vec![
        Vec2::new(-10.0, -10.0),
        Vec2::new(0.0, -10.0),
        Vec2::new(0.0, 10.0),
        Vec2::new(-10.0, 10.0),
    ]));
    let right = Shape::Polygon(Polygon::new(vec![
        Vec2::new(0.0, -10.0),
        Vec2::new(10.0, -10.0),
        Vec2::new(10.0, 10.0),
        Vec2::new(0.0, 10.0),
    ]));
    Compound::new(vec![left, right])
}

#[test]
fn compound_reports_the_part_actually_struck() {
    let compound = two_cell_compound();
    let shape = Shape::Compound(compound);
    let bullet = Shape::Circle { radius: 3.0 };

    // Bullet sits just left of centre ⇒ should hit part 0 (the left cell), not part 1.
    let c = intersects(Vec2::ZERO, 0.0, &shape, Vec2::new(-4.0, 0.0), 0.0, &bullet).unwrap();
    assert_eq!(
        c.part_a, 0,
        "expected the left cell (part 0) to register the hit"
    );

    let c2 = intersects(Vec2::ZERO, 0.0, &shape, Vec2::new(4.0, 0.0), 0.0, &bullet).unwrap();
    assert_eq!(
        c2.part_a, 1,
        "expected the right cell (part 1) to register the hit"
    );
}

#[test]
fn disabled_compound_part_is_invisible_to_every_test() {
    let mut compound = two_cell_compound();
    compound.disable_part(0);
    let shape = Shape::Compound(compound);
    let bullet = Shape::Circle { radius: 3.0 };

    // Bullet inside the now-disabled left cell: no contact at all.
    assert!(intersects(Vec2::ZERO, 0.0, &shape, Vec2::new(-4.0, 0.0), 0.0, &bullet).is_none());
    // Right cell still collides normally.
    assert!(intersects(Vec2::ZERO, 0.0, &shape, Vec2::new(4.0, 0.0), 0.0, &bullet).is_some());
}

#[test]
fn collect_contacts_returns_one_per_overlapping_part() {
    let compound = two_cell_compound();
    // A tall thin bar overlapping BOTH cells (spans x=-2..2).
    let bar = square(2.0);
    let mut out = Vec::new();
    collect_contacts(Vec2::ZERO, 0.0, &compound, Vec2::ZERO, 0.0, &bar, &mut out);
    assert_eq!(out.len(), 2, "the bar overlaps both cells");
    let parts: Vec<i32> = {
        let mut p: Vec<i32> = out.iter().map(|c| c.part_a).collect();
        p.sort();
        p
    };
    assert_eq!(parts, vec![0, 1]);
}

// --- raycasts ------------------------------------------------------------

#[test]
fn raycast_hits_a_circle_head_on() {
    let circle = Shape::Circle { radius: 5.0 };
    let hit = raycast(
        Vec2::new(-100.0, 0.0),
        Vec2::X,
        1000.0,
        Vec2::ZERO,
        0.0,
        &circle,
    )
    .unwrap();
    assert!((hit.distance - 95.0).abs() < 1e-3);
    assert!((hit.point - Vec2::new(-5.0, 0.0)).length() < 1e-3);
}

#[test]
fn raycast_misses_a_circle_that_is_not_in_the_way() {
    let circle = Shape::Circle { radius: 5.0 };
    assert!(raycast(
        Vec2::new(-100.0, 100.0),
        Vec2::X,
        1000.0,
        Vec2::ZERO,
        0.0,
        &circle
    )
    .is_none());
}

#[test]
fn raycast_hits_the_nearest_compound_part() {
    let compound = two_cell_compound();
    let shape = Shape::Compound(compound);
    let hit = raycast(
        Vec2::new(-1000.0, 0.0),
        Vec2::X,
        2000.0,
        Vec2::ZERO,
        0.0,
        &shape,
    )
    .unwrap();
    assert_eq!(hit.part_index, 0, "ray enters the left cell first");
}
