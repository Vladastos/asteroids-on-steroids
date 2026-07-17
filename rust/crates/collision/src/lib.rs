//! Collision detection — Rust port of `GameEngine/Engine/Collision/`.
//!
//! Pure: `glam` only, no Bevy, no ECS. Broad phase ([`SpatialGrid`]) is generic
//! over an opaque handle so it doesn't need to know about Bevy's `Entity`.
//!
//! Narrow-phase impulse RESOLUTION (the semantic response: damage, splits,
//! score) deliberately stays out of this crate — it reads `RigidBody`/`Velocity`
//! and belongs in a `game` crate `FixedUpdate` system, exactly as the C#'s
//! `CollisionSystem` sits above these shapes.

mod contact;
mod geom;
mod shape;
mod spatial_grid;

pub use contact::{ContactInfo, RayCastResult};
pub use geom::{compute_area, compute_centroid, nearest_point_on_boundary, point_in_polygon};
pub use shape::{collect_contacts, get_aabb, intersects, raycast, Compound, Polygon, Shape};
pub use spatial_grid::SpatialGrid;
