//! Port of `GameConfig/Models/PlayerConfig.cs`.

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlayerConfig {
    pub shape: String,
    /// Uniform scale applied to authored shape vertices.
    pub shape_scale: f32,
    /// Optional material override; empty = use the shape's own material.
    pub material: String,
    /// px/s² acceleration rate.
    pub thrust: f32,
    /// rad/s.
    pub rot_speed: f32,
    /// px/s.
    pub max_speed: f32,
    /// Exponential decay rate s⁻¹ when no keys held.
    pub brake_drag: f32,
    /// px/s velocity burst on key press.
    pub impulse: f32,
    /// s⁻¹ bleed on aim-perpendicular velocity.
    pub lateral_drag: f32,
    pub starting_weapon: String,
    pub skills: Vec<String>,
    /// All impact energy targeting the player is multiplied by this. Primary
    /// tuning lever for ship durability — lower = tankier.
    pub player_impact_coeff: f32,
    /// Thrust multiplier when some (but not all) propeller cells are alive.
    pub thrust_partial_mult: f32,
    pub vortex_centripetal: f32,
    pub vortex_tangential: f32,
}

impl Default for PlayerConfig {
    fn default() -> Self {
        Self {
            shape: "player_ship".into(),
            shape_scale: 0.5,
            material: String::new(),
            thrust: 4500.0,
            rot_speed: 3.2,
            max_speed: 900.0,
            brake_drag: 4.0,
            impulse: 250.0,
            lateral_drag: 6.0,
            starting_weapon: "cannon".into(),
            skills: vec!["dash".into(), "turbo".into(), "slowmo".into()],
            player_impact_coeff: 0.4,
            thrust_partial_mult: 0.6,
            vortex_centripetal: 0.5,
            vortex_tangential: 0.5,
        }
    }
}
