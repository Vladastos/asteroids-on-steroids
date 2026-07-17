//! Port of `ContactInfo.cs` / `RayCastResult.cs`.

use glam::Vec2;

/// Result of a successful narrow-phase test. `normal` points from B into A (the
/// direction A must move to separate); `depth` is the overlap along it.
#[derive(Clone, Copy, Debug)]
pub struct ContactInfo {
    pub normal: Vec2,
    pub depth: f32,
    pub contact_point: Vec2,
    /// Touching part (cell) index on A / B when that side is a `Compound`; -1 unknown.
    pub part_a: i32,
    pub part_b: i32,
}

impl ContactInfo {
    pub fn new(normal: Vec2, depth: f32, contact_point: Vec2) -> Self {
        Self {
            normal,
            depth,
            contact_point,
            part_a: -1,
            part_b: -1,
        }
    }

    pub fn flipped(self) -> Self {
        Self {
            normal: -self.normal,
            ..self
        }
    }

    pub fn with_parts(self, part_a: i32, part_b: i32) -> Self {
        Self {
            part_a,
            part_b,
            ..self
        }
    }
}

/// Result of a shape-level raycast. Port of `RayCastResult.cs`.
#[derive(Clone, Copy, Debug)]
pub struct RayCastResult {
    pub distance: f32,
    pub point: Vec2,
    pub normal: Vec2,
    /// Compound part struck; -1 for simple shapes.
    pub part_index: i32,
}
