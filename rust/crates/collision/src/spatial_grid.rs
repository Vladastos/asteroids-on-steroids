//! Uniform spatial hash grid — port of `SpatialGrid.cs`. Generic over an opaque
//! handle `H` (Copy + Eq) so this crate doesn't need to know about Bevy's
//! `Entity`; the `game` crate instantiates `SpatialGrid<Entity>`.

use glam::Vec2;
use std::collections::HashMap;

/// Rebuilt from scratch each frame: `clear()` then `insert()` per body, then
/// queried with `candidates()`. Cell size should be ≈1.5× the diameter of the
/// largest commonly-spawned entity.
pub struct SpatialGrid<H> {
    cell_size: f32,
    cells: HashMap<i64, Vec<H>>,
}

impl<H: Copy + Eq> SpatialGrid<H> {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size,
            cells: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.cells.clear();
    }

    pub fn insert(&mut self, handle: H, min: Vec2, max: Vec2) {
        let (min_cx, min_cy) = self.cell_coord(min);
        let (max_cx, max_cy) = self.cell_coord(max);
        for cx in min_cx..=max_cx {
            for cy in min_cy..=max_cy {
                self.cells
                    .entry(Self::cell_key(cx, cy))
                    .or_default()
                    .push(handle);
            }
        }
    }

    /// Entities sharing at least one cell with the query AABB. May include false
    /// positives — narrow phase filters them. Deduplicated.
    pub fn candidates(&self, min: Vec2, max: Vec2, results: &mut Vec<H>) {
        let (min_cx, min_cy) = self.cell_coord(min);
        let (max_cx, max_cy) = self.cell_coord(max);
        for cx in min_cx..=max_cx {
            for cy in min_cy..=max_cy {
                if let Some(list) = self.cells.get(&Self::cell_key(cx, cy)) {
                    for &h in list {
                        if !results.contains(&h) {
                            results.push(h);
                        }
                    }
                }
            }
        }
    }

    fn cell_coord(&self, v: Vec2) -> (i32, i32) {
        (
            (v.x / self.cell_size).floor() as i32,
            (v.y / self.cell_size).floor() as i32,
        )
    }

    fn cell_key(cx: i32, cy: i32) -> i64 {
        ((cx as u32 as i64) << 32) | (cy as u32 as i64)
    }
}

impl<H: Copy + Eq> Default for SpatialGrid<H> {
    fn default() -> Self {
        Self::new(128.0)
    }
}
