//! Polygon geometry helpers — ports of the pieces of `PolygonUtils.cs` and the
//! point-location helpers in `FractureService.cs` that the fracture core needs.

use crate::types::FracturableBody;
use glam::Vec2;

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
            && p.x < (poly[j].x - poly[i].x) * (p.y - poly[i].y) / (poly[j].y - poly[i].y) + poly[i].x
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
