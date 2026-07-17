//! Polygon geometry helpers — ports of the pieces of `PolygonUtils.cs` and the
//! point-location helpers in `FractureService.cs` that the fracture core needs.
//! (Duplicates a couple of functions also in the `collision` crate — kept
//! separate so `fracture` has zero cross-crate coupling, per MIGRATION.md.)

use crate::types::FracturableBody;
use glam::Vec2;

/// Signed area via the shoelace formula. Positive → CCW in Y-up math (CW in the
/// engine's Y-down screen space). Port of `PolygonUtils.ComputeArea`.
pub fn compute_area(verts: &[Vec2]) -> f32 {
    let n = verts.len();
    let mut area = 0.0f32;
    for i in 0..n {
        let a = verts[i];
        let b = verts[(i + 1) % n];
        area += a.x * b.y - b.x * a.y;
    }
    area * 0.5
}

/// Area centroid via the shoelace triangulation formula. Port of `ComputeCentroid`.
pub fn compute_centroid(verts: &[Vec2]) -> Vec2 {
    let n = verts.len();
    let mut c = Vec2::ZERO;
    let mut area = 0.0f32;
    for i in 0..n {
        let a = verts[i];
        let b = verts[(i + 1) % n];
        let cross = a.x * b.y - b.x * a.y;
        area += cross;
        c += (a + b) * cross;
    }
    area *= 0.5;
    if area.abs() < 1e-6 {
        let sum: Vec2 = verts.iter().copied().sum();
        return sum / n as f32;
    }
    c / (6.0 * area)
}

/// Clips a convex polygon against a half-plane, keeping the side where
/// `dot(p - plane_point, plane_normal) >= 0`. Port of `ClipConvexByHalfPlane`.
pub fn clip_convex_by_half_plane(
    polygon: &[Vec2],
    plane_point: Vec2,
    plane_normal: Vec2,
) -> Vec<Vec2> {
    let n = polygon.len();
    if n == 0 {
        return Vec::new();
    }
    let mut output = Vec::with_capacity(n + 1);
    for i in 0..n {
        let curr = polygon[i];
        let next = polygon[(i + 1) % n];
        let d_curr = (curr - plane_point).dot(plane_normal);
        let d_next = (next - plane_point).dot(plane_normal);
        let curr_in = d_curr >= 0.0;
        let next_in = d_next >= 0.0;
        if curr_in {
            output.push(curr);
        }
        if curr_in != next_in {
            let t = d_curr / (d_curr - d_next);
            output.push(curr + t * (next - curr));
        }
    }
    output
}

/// Re-centres world-space vertices around their area centroid. Returns
/// `(centroid, centroid-relative vertices)`. Port of `RecenterVertices`.
pub fn recenter_vertices(world_vertices: &[Vec2]) -> (Vec2, Vec<Vec2>) {
    let centroid = compute_centroid(world_vertices);
    (
        centroid,
        world_vertices.iter().map(|&v| v - centroid).collect(),
    )
}

/// Polar moment of inertia of a convex polygon about its centroid, for a body of
/// the given `mass`. Verts are centroid-relative. Port of `PolygonUtils.ComputeInertia`.
pub fn compute_inertia(centroid_relative_verts: &[Vec2], mass: f32) -> f32 {
    let n = centroid_relative_verts.len();
    if n < 3 {
        return 0.0;
    }
    let mut signed2 = 0.0f32; // 2× signed area
    for i in 0..n {
        let a = centroid_relative_verts[i];
        let b = centroid_relative_verts[(i + 1) % n];
        signed2 += a.x * b.y - b.x * a.y;
    }
    let area = (signed2 * 0.5).abs();
    if area < 1e-6 {
        return 0.0;
    }
    let density = mass / area;
    let mut inertia = 0.0f32;
    for i in 0..n {
        let a = centroid_relative_verts[i];
        let b = centroid_relative_verts[(i + 1) % n];
        let cross = (a.x * b.y - b.x * a.y).abs();
        inertia += cross * (a.dot(a) + a.dot(b) + b.dot(b));
    }
    density * inertia / 12.0
}

/// Even-odd point-in-polygon. Port of `FractureService.ContainsPoint`.
pub fn contains_point(poly: &[Vec2], p: Vec2) -> bool {
    let n = poly.len();
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        if (poly[i].y > p.y) != (poly[j].y > p.y)
            && p.x
                < (poly[j].x - poly[i].x) * (p.y - poly[i].y) / (poly[j].y - poly[i].y) + poly[i].x
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Min distance from `p` to a polygon's boundary. Port of `DistanceToPolygon`.
pub fn distance_to_polygon(poly: &[Vec2], p: Vec2) -> f32 {
    let n = poly.len();
    let mut best = f32::MAX;
    let mut j = n - 1;
    for i in 0..n {
        let a = poly[j];
        let b = poly[i];
        let ab = b - a;
        let len2 = ab.length_squared();
        let t = if len2 > 1e-9 {
            ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let dist = (p - (a + ab * t)).length();
        if dist < best {
            best = dist;
        }
        j = i;
    }
    best
}

/// The cell an impact lands on, in the body's local frame: the containing cell,
/// else the nearest by polygon distance. Port of `FractureService.NearestCell`.
pub fn nearest_cell(body: &FracturableBody, pos: Vec2, rot: f32, world_point: Vec2) -> usize {
    let (sin, cos) = rot.sin_cos();
    let d = world_point - pos;
    let local = Vec2::new(d.x * cos + d.y * sin, -d.x * sin + d.y * cos); // un-rotate
    let mut best = 0usize;
    let mut best_dist = f32::MAX;
    for (i, cell) in body.cells.iter().enumerate() {
        if contains_point(&cell.local, local) {
            return i;
        }
        let dist = distance_to_polygon(&cell.local, local);
        if dist < best_dist {
            best_dist = dist;
            best = i;
        }
    }
    best
}
