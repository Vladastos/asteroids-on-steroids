//! Shared polygon geometry — port of the pieces of `PolygonUtils.cs` collision
//! needs (fracture has its own copy of the ones it needs, to keep the crates
//! independent; this trades a little duplication for zero cross-crate coupling).

use glam::Vec2;

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

/// Even-odd point-in-polygon.
pub fn point_in_polygon(p: Vec2, poly: &[Vec2]) -> bool {
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

/// Nearest point on the polygon boundary to `point`. Port of `NearestPointOnBoundary`.
pub fn nearest_point_on_boundary(polygon: &[Vec2], point: Vec2) -> Vec2 {
    let n = polygon.len();
    let mut best_dist = f32::MAX;
    let mut best = polygon[0];
    for i in 0..n {
        let a = polygon[i];
        let b = polygon[(i + 1) % n];
        let ab = b - a;
        let len_sq = ab.length_squared();
        let t = if len_sq > 1e-10 {
            ((point - a).dot(ab) / len_sq).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let proj = a + ab * t;
        let d = (proj - point).length_squared();
        if d < best_dist {
            best_dist = d;
            best = proj;
        }
    }
    best
}
