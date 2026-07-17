//! Collision shape hierarchy — port of `CollisionShape.cs` + `CircleShape.cs` +
//! `AABBShape.cs` + `PolygonShape.cs` + `CompoundShape.cs`.
//!
//! The C# uses an abstract class + double-dispatch (`A.Intersects(B)` →
//! `B.IntersectsCircle/Polygon/AABB(A)`) so each shape pair gets its own
//! algorithm. Rust has no open class hierarchy here (and doesn't need one — the
//! shape set is closed), so this is a `Shape` enum dispatched with `match`,
//! per MIGRATION.md's "prefer enum over trait objects" call: cache-friendlier,
//! no `dyn`, and the match arms are exactly the C#'s double-dispatch table
//! flattened into one place instead of scattered across 4 classes.
//!
//! Contact convention (matches `ContactInfo.cs`): `normal` points from B into A —
//! the direction A must move to separate.

use crate::contact::{ContactInfo, RayCastResult};
use glam::Vec2;

/// A convex collision shape, or a compound of them (e.g. an asteroid's cells).
/// Polygon vertices are local-space (centroid-relative); AABB half-extents ignore
/// rotation; Compound children share the entity's pose and are expressed in the
/// compound's local space, matching `PolygonShape`'s convention.
#[derive(Clone, Debug)]
pub enum Shape {
    Circle { radius: f32 },
    Aabb { half_width: f32, half_height: f32 },
    Polygon(Polygon),
    Compound(Compound),
}

#[derive(Clone, Debug)]
pub struct Polygon {
    /// Local-space vertices, centred at the origin. Must be convex, clockwise
    /// winding (engine convention, Y-down).
    pub local_vertices: Vec<Vec2>,
}

impl Polygon {
    pub fn new(local_vertices: Vec<Vec2>) -> Self {
        assert!(
            local_vertices.len() >= 3,
            "a polygon needs at least 3 vertices"
        );
        Self { local_vertices }
    }

    pub fn world_vertices(&self, pos: Vec2, rot: f32) -> Vec<Vec2> {
        transform_vertices(&self.local_vertices, pos, rot)
    }
}

/// A shape composed of convex child shapes sharing the entity's pose, with a
/// per-part local AABB precomputed for broad-phase culling. Port of `CompoundShape`.
#[derive(Clone, Debug)]
pub struct Compound {
    pub parts: Vec<Shape>,
    local_aabbs: Vec<(Vec2, Vec2)>,
    local_bounds: (Vec2, Vec2),
    /// Pulverised cells: permanently skipped by every test (the hole a fractured
    /// cell leaves). Port of `CompoundShape.DisablePart`.
    pub disabled: Vec<bool>,
}

impl Compound {
    pub fn new(parts: Vec<Shape>) -> Self {
        assert!(!parts.is_empty(), "a compound needs at least one part");
        let local_aabbs: Vec<_> = parts.iter().map(|p| get_aabb(p, Vec2::ZERO, 0.0)).collect();
        let mut bmin = Vec2::splat(f32::MAX);
        let mut bmax = Vec2::splat(f32::MIN);
        for &(min, max) in &local_aabbs {
            bmin = bmin.min(min);
            bmax = bmax.max(max);
        }
        let disabled = vec![false; parts.len()];
        Self {
            parts,
            local_aabbs,
            local_bounds: (bmin, bmax),
            disabled,
        }
    }

    pub fn disable_part(&mut self, index: usize) {
        if index < self.disabled.len() {
            self.disabled[index] = true;
        }
    }

    pub fn is_part_disabled(&self, index: usize) -> bool {
        self.disabled.get(index).copied().unwrap_or(false)
    }
}

fn transform_vertices(local: &[Vec2], pos: Vec2, rot: f32) -> Vec<Vec2> {
    let (sin, cos) = rot.sin_cos();
    local
        .iter()
        .map(|v| Vec2::new(v.x * cos - v.y * sin + pos.x, v.x * sin + v.y * cos + pos.y))
        .collect()
}

fn aabb_corners(pos: Vec2, half_width: f32, half_height: f32) -> (Vec2, Vec2) {
    (
        pos - Vec2::new(half_width, half_height),
        pos + Vec2::new(half_width, half_height),
    )
}

fn aabb_as_polygon_verts(pos: Vec2, half_width: f32, half_height: f32) -> Vec<Vec2> {
    vec![
        Vec2::new(pos.x - half_width, pos.y - half_height),
        Vec2::new(pos.x + half_width, pos.y - half_height),
        Vec2::new(pos.x + half_width, pos.y + half_height),
        Vec2::new(pos.x - half_width, pos.y + half_height),
    ]
}

/// AABB for broad-phase culling. Port of each shape's `GetAABB`.
pub fn get_aabb(shape: &Shape, pos: Vec2, rot: f32) -> (Vec2, Vec2) {
    match shape {
        Shape::Circle { radius } => (pos - Vec2::splat(*radius), pos + Vec2::splat(*radius)),
        Shape::Aabb {
            half_width,
            half_height,
        } => aabb_corners(pos, *half_width, *half_height),
        Shape::Polygon(p) => {
            let verts = p.world_vertices(pos, rot);
            let mut min = Vec2::splat(f32::MAX);
            let mut max = Vec2::splat(f32::MIN);
            for v in verts {
                min = min.min(v);
                max = max.max(v);
            }
            (min, max)
        }
        Shape::Compound(c) => {
            // O(1): transform the four corners of the precomputed local bounds.
            let (lmin, lmax) = c.local_bounds;
            let (sin, cos) = rot.sin_cos();
            let to_world =
                |l: Vec2| Vec2::new(l.x * cos - l.y * sin + pos.x, l.x * sin + l.y * cos + pos.y);
            let c0 = to_world(lmin);
            let c1 = to_world(Vec2::new(lmax.x, lmin.y));
            let c2 = to_world(Vec2::new(lmin.x, lmax.y));
            let c3 = to_world(lmax);
            (c0.min(c1).min(c2).min(c3), c0.max(c1).max(c2).max(c3))
        }
    }
}

fn aabb_overlap(a: (Vec2, Vec2), bmin: Vec2, bmax: Vec2) -> bool {
    a.0.x <= bmax.x && a.1.x >= bmin.x && a.0.y <= bmax.y && a.1.y >= bmin.y
}

// ---------------------------------------------------------------------------
// Narrow phase: pairwise primitives. Each returns a contact whose normal
// convention is documented per function; `intersects` composes them into the
// "normal points B→A" convention the crate promises.
// ---------------------------------------------------------------------------

/// Circle vs circle. Normal points from B's centre toward A's centre.
fn circle_vs_circle(pos_a: Vec2, r_a: f32, pos_b: Vec2, r_b: f32) -> Option<ContactInfo> {
    let diff = pos_a - pos_b;
    let dist_sq = diff.length_squared();
    let rad_sum = r_a + r_b;
    if dist_sq >= rad_sum * rad_sum {
        return None;
    }
    let dist = dist_sq.sqrt();
    let normal = if dist > 1e-6 { diff / dist } else { Vec2::X };
    let depth = rad_sum - dist;
    let contact = pos_a - normal * r_a;
    Some(ContactInfo::new(normal, depth, contact))
}

/// Circle vs axis-aligned box (closest-point method). Normal points from the
/// box's surface toward the circle's centre.
fn circle_vs_aabb(
    circle_pos: Vec2,
    radius: f32,
    aabb_pos: Vec2,
    half: Vec2,
) -> Option<ContactInfo> {
    let local = circle_pos - aabb_pos;
    let clamped = local.clamp(-half, half);
    let closest = aabb_pos + clamped;
    let diff = circle_pos - closest;
    let dist_sq = diff.length_squared();
    if dist_sq >= radius * radius {
        return None;
    }
    let dist = dist_sq.sqrt();
    let normal = if dist > 1e-6 { diff / dist } else { Vec2::Y };
    let depth = radius - dist;
    Some(ContactInfo::new(normal, depth, closest))
}

fn project(verts: &[Vec2], axis: Vec2) -> (f32, f32) {
    let mut min = verts[0].dot(axis);
    let mut max = min;
    for &v in &verts[1..] {
        let p = v.dot(axis);
        min = min.min(p);
        max = max.max(p);
    }
    (min, max)
}

fn centroid(verts: &[Vec2]) -> Vec2 {
    verts.iter().copied().sum::<Vec2>() / verts.len() as f32
}

fn nearest_vertex(verts: &[Vec2], point: Vec2) -> Vec2 {
    let mut best = verts[0];
    let mut best_sq = (verts[0] - point).length_squared();
    for &v in &verts[1..] {
        let sq = (v - point).length_squared();
        if sq < best_sq {
            best_sq = sq;
            best = v;
        }
    }
    best
}

/// SAT: circle vs convex polygon. Normal points from the polygon toward the
/// circle. Port of `PolygonShape.SatCirclePolygon`.
fn sat_circle_polygon(
    verts: &[Vec2],
    poly_pos: Vec2,
    radius: f32,
    circle_pos: Vec2,
) -> Option<ContactInfo> {
    let mut min_depth = f32::MAX;
    let mut min_normal = Vec2::ZERO;
    let n = verts.len();
    for i in 0..n {
        let edge = verts[(i + 1) % n] - verts[i];
        let axis = Vec2::new(-edge.y, edge.x).normalize();
        let (min_p, max_p) = project(verts, axis);
        let circle_proj = circle_pos.dot(axis);
        let (min_c, max_c) = (circle_proj - radius, circle_proj + radius);
        let overlap = max_p.min(max_c) - min_p.max(min_c);
        if overlap <= 0.0 {
            return None;
        }
        if overlap < min_depth {
            min_depth = overlap;
            min_normal = axis;
        }
    }

    let nearest = nearest_vertex(verts, circle_pos);
    let vert_axis_raw = circle_pos - nearest;
    if vert_axis_raw.length_squared() > 1e-10 {
        let vert_axis = vert_axis_raw.normalize();
        let (min_p, max_p) = project(verts, vert_axis);
        let circle_proj = circle_pos.dot(vert_axis);
        let (min_c, max_c) = (circle_proj - radius, circle_proj + radius);
        let overlap = max_p.min(max_c) - min_p.max(min_c);
        if overlap <= 0.0 {
            return None;
        }
        if overlap < min_depth {
            min_depth = overlap;
            min_normal = vert_axis;
        }
    }

    if min_normal.dot(circle_pos - poly_pos) < 0.0 {
        min_normal = -min_normal;
    }
    let contact = circle_pos - min_normal * radius;
    Some(ContactInfo::new(min_normal, min_depth, contact))
}

fn test_axes(
    a: &[Vec2],
    b: &[Vec2],
    axis_source: &[Vec2],
    min_depth: &mut f32,
    min_normal: &mut Vec2,
) -> bool {
    let n = axis_source.len();
    for i in 0..n {
        let edge = axis_source[(i + 1) % n] - axis_source[i];
        let axis = Vec2::new(-edge.y, edge.x).normalize();
        let (min_a, max_a) = project(a, axis);
        let (min_b, max_b) = project(b, axis);
        let overlap = max_a.min(max_b) - min_a.max(min_b);
        if overlap <= 0.0 {
            return false;
        }
        if overlap < *min_depth {
            *min_depth = overlap;
            *min_normal = axis;
        }
    }
    true
}

/// Approximates the contact point as B's deepest vertex along `-normal`.
fn find_contact_point(b: &[Vec2], normal: Vec2) -> Vec2 {
    let mut best = b[0];
    let mut best_dot = b[0].dot(-normal);
    for &v in &b[1..] {
        let d = v.dot(-normal);
        if d > best_dot {
            best_dot = d;
            best = v;
        }
    }
    best
}

/// SAT: convex polygon vs convex polygon. Normal points from B toward A.
/// Port of `PolygonShape.SatPolygonPolygon`.
fn sat_polygon_polygon(a: &[Vec2], b: &[Vec2]) -> Option<ContactInfo> {
    let mut min_depth = f32::MAX;
    let mut min_normal = Vec2::ZERO;
    if !test_axes(a, b, a, &mut min_depth, &mut min_normal) {
        return None;
    }
    if !test_axes(a, b, b, &mut min_depth, &mut min_normal) {
        return None;
    }
    let (ca, cb) = (centroid(a), centroid(b));
    if min_normal.dot(ca - cb) < 0.0 {
        min_normal = -min_normal;
    }
    let contact = find_contact_point(b, min_normal);
    Some(ContactInfo::new(min_normal, min_depth, contact))
}

// ---------------------------------------------------------------------------
// Narrow phase: shape-pair entry point. Recurses into Compound on either side.
// ---------------------------------------------------------------------------

/// Tests whether shape `a` (at `pos_a`/`rot_a`) overlaps shape `b`. Returns the
/// deepest contact; `normal` points from B into A. `part_a`/`part_b` on the
/// result identify which Compound child (if any) produced it — the game layer
/// uses this to seed fracture on the cell actually struck, not a guess from the
/// contact point.
pub fn intersects(
    pos_a: Vec2,
    rot_a: f32,
    a: &Shape,
    pos_b: Vec2,
    rot_b: f32,
    b: &Shape,
) -> Option<ContactInfo> {
    match (a, b) {
        (Shape::Compound(ca), _) => {
            let (wmin, wmax) = get_aabb(b, pos_b, rot_b);
            let (qmin, qmax) = world_aabb_to_local(wmin, wmax, pos_a, rot_a);
            let mut deepest: Option<ContactInfo> = None;
            for (i, part) in ca.parts.iter().enumerate() {
                if ca.disabled[i] || !aabb_overlap(ca.local_aabbs[i], qmin, qmax) {
                    continue;
                }
                if let Some(c) = intersects(pos_a, rot_a, part, pos_b, rot_b, b) {
                    if deepest.is_none_or(|d| c.depth > d.depth) {
                        deepest = Some(c.with_parts(i as i32, c.part_b));
                    }
                }
            }
            deepest
        }
        (_, Shape::Compound(cb)) => {
            let (wmin, wmax) = get_aabb(a, pos_a, rot_a);
            let (qmin, qmax) = world_aabb_to_local(wmin, wmax, pos_b, rot_b);
            let mut deepest: Option<ContactInfo> = None;
            for (i, part) in cb.parts.iter().enumerate() {
                if cb.disabled[i] || !aabb_overlap(cb.local_aabbs[i], qmin, qmax) {
                    continue;
                }
                if let Some(c) = intersects(pos_a, rot_a, a, pos_b, rot_b, part) {
                    if deepest.is_none_or(|d| c.depth > d.depth) {
                        deepest = Some(c.with_parts(c.part_a, i as i32));
                    }
                }
            }
            deepest
        }
        (Shape::Circle { radius: r_a }, Shape::Circle { radius: r_b }) => {
            circle_vs_circle(pos_a, *r_a, pos_b, *r_b)
        }
        (
            Shape::Circle { radius },
            Shape::Aabb {
                half_width,
                half_height,
            },
        ) => {
            // base points box(B)'s surface toward circle(A) = B→A already.
            circle_vs_aabb(pos_a, *radius, pos_b, Vec2::new(*half_width, *half_height))
        }
        (
            Shape::Aabb {
                half_width,
                half_height,
            },
            Shape::Circle { radius },
        ) => {
            // base points box→circle = from A(box)'s surface toward B(circle) = A→B; we need B→A, so flip.
            circle_vs_aabb(pos_b, *radius, pos_a, Vec2::new(*half_width, *half_height))
                .map(|c| c.flipped())
        }
        (Shape::Circle { radius }, Shape::Polygon(p)) => {
            let verts = p.world_vertices(pos_b, rot_b);
            // base points polygon(B)→circle(A) = B→A already.
            sat_circle_polygon(&verts, pos_b, *radius, pos_a)
        }
        (Shape::Polygon(p), Shape::Circle { radius }) => {
            let verts = p.world_vertices(pos_a, rot_a);
            // base points polygon(A)→circle(B) = A→B; flip for B→A.
            sat_circle_polygon(&verts, pos_a, *radius, pos_b).map(|c| c.flipped())
        }
        (Shape::Polygon(pa), Shape::Polygon(pb)) => {
            let va = pa.world_vertices(pos_a, rot_a);
            let vb = pb.world_vertices(pos_b, rot_b);
            sat_polygon_polygon(&va, &vb)
        }
        (
            Shape::Aabb {
                half_width,
                half_height,
            },
            Shape::Polygon(p),
        ) => {
            let va = aabb_as_polygon_verts(pos_a, *half_width, *half_height);
            let vb = p.world_vertices(pos_b, rot_b);
            sat_polygon_polygon(&va, &vb)
        }
        (
            Shape::Polygon(p),
            Shape::Aabb {
                half_width,
                half_height,
            },
        ) => {
            let va = p.world_vertices(pos_a, rot_a);
            let vb = aabb_as_polygon_verts(pos_b, *half_width, *half_height);
            sat_polygon_polygon(&va, &vb)
        }
        (
            Shape::Aabb {
                half_width: hwa,
                half_height: hha,
            },
            Shape::Aabb {
                half_width: hwb,
                half_height: hhb,
            },
        ) => aabb_vs_aabb(pos_a, Vec2::new(*hwa, *hha), pos_b, Vec2::new(*hwb, *hhb)),
    }
}

/// AABB vs AABB — the minimum-translation axis. Normal points from B toward A.
/// (The C# uses AABB mainly as a UI / broad-phase stand-in, not a gameplay
/// shape; this crate gives it a clean, self-consistent, tested convention
/// rather than the C#'s double-dispatch quirks, which barely matter here.)
fn aabb_vs_aabb(pos_a: Vec2, half_a: Vec2, pos_b: Vec2, half_b: Vec2) -> Option<ContactInfo> {
    let overlap_x = (half_a.x + half_b.x) - (pos_a.x - pos_b.x).abs();
    let overlap_y = (half_a.y + half_b.y) - (pos_a.y - pos_b.y).abs();
    if overlap_x <= 0.0 || overlap_y <= 0.0 {
        return None;
    }
    let (normal, depth) = if overlap_x < overlap_y {
        (
            if pos_a.x < pos_b.x { -Vec2::X } else { Vec2::X },
            overlap_x,
        )
    } else {
        (
            if pos_a.y < pos_b.y { -Vec2::Y } else { Vec2::Y },
            overlap_y,
        )
    };
    let contact = pos_a - normal * (depth * 0.5);
    Some(ContactInfo::new(normal, depth, contact))
}

fn world_aabb_to_local(wmin: Vec2, wmax: Vec2, pos: Vec2, rot: f32) -> (Vec2, Vec2) {
    let (sin, cos) = rot.sin_cos();
    let to_local = |w: Vec2| {
        let d = w - pos;
        Vec2::new(d.x * cos + d.y * sin, -d.x * sin + d.y * cos)
    };
    let c0 = to_local(wmin);
    let c1 = to_local(Vec2::new(wmax.x, wmin.y));
    let c2 = to_local(Vec2::new(wmin.x, wmax.y));
    let c3 = to_local(wmax);
    (c0.min(c1).min(c2).min(c3), c0.max(c1).max(c2).max(c3))
}

// ---------------------------------------------------------------------------
// Contact manifold: every Compound part that overlaps `other`, not just the
// deepest. Port of `CompoundShape.CollectContacts`.
// ---------------------------------------------------------------------------

/// Collects one contact per part of `compound` (at `pos_a`/`rot_a`) that
/// overlaps `other`. Each normal is oriented by the touching CELL's geometry
/// (not the compound's centroid), so concavities separate correctly — a body
/// sitting in a crater is pushed out of the wall it actually touches.
pub fn collect_contacts(
    pos_a: Vec2,
    rot_a: f32,
    compound: &Compound,
    pos_b: Vec2,
    rot_b: f32,
    other: &Shape,
    out: &mut Vec<ContactInfo>,
) {
    let (wmin, wmax) = get_aabb(other, pos_b, rot_b);
    let (qmin, qmax) = world_aabb_to_local(wmin, wmax, pos_a, rot_a);
    let (sin, cos) = rot_a.sin_cos();
    for (i, part) in compound.parts.iter().enumerate() {
        if compound.disabled[i] || !aabb_overlap(compound.local_aabbs[i], qmin, qmax) {
            continue;
        }
        let Some(c) = intersects(pos_a, rot_a, part, pos_b, rot_b, other) else {
            continue;
        };

        let other_part = if let Shape::Compound(oc) = other {
            find_hit_part(oc, pos_b, rot_b, pos_a, rot_a, part)
        } else {
            -1
        };

        let (amin, amax) = compound.local_aabbs[i];
        let lc = (amin + amax) * 0.5;
        let cell_world = Vec2::new(
            lc.x * cos - lc.y * sin + pos_a.x,
            lc.x * sin + lc.y * cos + pos_a.y,
        );
        let mut ci = c.with_parts(i as i32, other_part);
        if ci.normal.dot(pos_b - cell_world) < 0.0 {
            ci = ci.flipped();
        }
        out.push(ci);
    }
}

/// Which part of `other` a single `probe` shape hits deepest (mirrors the C#'s
/// `LastHitPartIndex` bookkeeping, computed on demand instead of as shared
/// mutable state).
fn find_hit_part(
    other: &Compound,
    pos_b: Vec2,
    rot_b: f32,
    pos_probe: Vec2,
    rot_probe: f32,
    probe: &Shape,
) -> i32 {
    let mut best_depth = f32::MIN;
    let mut best = -1i32;
    for (i, part) in other.parts.iter().enumerate() {
        if other.disabled[i] {
            continue;
        }
        if let Some(c) = intersects(pos_probe, rot_probe, probe, pos_b, rot_b, part) {
            if c.depth > best_depth {
                best_depth = c.depth;
                best = i as i32;
            }
        }
    }
    best
}

// ---------------------------------------------------------------------------
// Raycasts. Port of each shape's `Raycast` override (AABB has none in the C#
// either — it falls back to "no hit").
// ---------------------------------------------------------------------------

pub fn raycast(
    origin: Vec2,
    dir: Vec2,
    max_dist: f32,
    pos: Vec2,
    rot: f32,
    shape: &Shape,
) -> Option<RayCastResult> {
    match shape {
        Shape::Circle { radius } => raycast_circle(origin, dir, max_dist, pos, *radius),
        Shape::Polygon(p) => raycast_polygon(origin, dir, max_dist, &p.world_vertices(pos, rot)),
        Shape::Aabb { .. } => None,
        Shape::Compound(c) => {
            let mut any = None;
            let mut best = max_dist;
            for (i, part) in c.parts.iter().enumerate() {
                if c.disabled[i] {
                    continue;
                }
                if let Some(h) = raycast(origin, dir, best, pos, rot, part) {
                    best = h.distance;
                    any = Some(RayCastResult {
                        part_index: i as i32,
                        ..h
                    });
                }
            }
            any
        }
    }
}

fn raycast_circle(
    origin: Vec2,
    dir: Vec2,
    max_dist: f32,
    pos: Vec2,
    radius: f32,
) -> Option<RayCastResult> {
    let m = origin - pos;
    let b = m.dot(dir);
    let c = m.dot(m) - radius * radius;
    if c > 0.0 && b > 0.0 {
        return None; // origin outside and pointing away
    }
    let disc = b * b - c;
    if disc < 0.0 {
        return None; // misses
    }
    let mut t = -b - disc.sqrt();
    if t < 0.0 {
        t = 0.0; // origin inside → hit at origin
    }
    if t > max_dist {
        return None;
    }
    let point = origin + dir * t;
    let normal = (point - pos).normalize();
    Some(RayCastResult {
        distance: t,
        point,
        normal,
        part_index: -1,
    })
}

/// Cyrus-Beck clip of the ray against the polygon's convex half-planes.
fn raycast_polygon(
    origin: Vec2,
    dir: Vec2,
    max_dist: f32,
    verts: &[Vec2],
) -> Option<RayCastResult> {
    let cen = centroid(verts);
    let n = verts.len();
    let mut t_enter = 0.0f32;
    let mut t_exit = max_dist;
    let mut enter_normal = Vec2::ZERO;
    let mut has_enter = false;

    for i in 0..n {
        let a = verts[i];
        let edge = verts[(i + 1) % n] - a;
        let mut nrm = Vec2::new(-edge.y, edge.x);
        if nrm.dot(a - cen) < 0.0 {
            nrm = -nrm;
        }
        nrm = nrm.normalize();

        let denom = dir.dot(nrm);
        if denom.abs() < 1e-9 {
            if (origin - a).dot(nrm) > 0.0 {
                return None; // parallel and outside
            }
            continue;
        }
        let t = (a - origin).dot(nrm) / denom;
        if denom < 0.0 {
            if t > t_enter {
                t_enter = t;
                enter_normal = nrm;
                has_enter = true;
            }
        } else if t < t_exit {
            t_exit = t;
        }
        if t_enter > t_exit {
            return None;
        }
    }

    if !has_enter || t_enter < 0.0 || t_enter > max_dist {
        return None;
    }
    let point = origin + dir * t_enter;
    Some(RayCastResult {
        distance: t_enter,
        point,
        normal: enter_normal,
        part_index: -1,
    })
}
