//! Port of `GameConfig/Models/PhysicsConfig.cs`.

use serde::Deserialize;

/// Default physics values applied to asteroid rigid bodies at spawn.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PhysicsConfig {
    pub linear_drag: f32,
    pub angular_drag: f32,
    pub restitution: f32,
    pub friction: f32,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            linear_drag: 0.05,
            angular_drag: 0.05,
            restitution: 0.30,
            friction: 0.20,
        }
    }
}
