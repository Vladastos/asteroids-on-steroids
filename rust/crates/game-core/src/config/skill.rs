//! Port of `GameConfig/Models/SkillConfig.cs`.

use serde::Deserialize;

/// Per-skill tuning. Type-specific fields are `Option`; only the relevant
/// subset is present for each skill in the JSON.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SkillConfig {
    pub cooldown: f32,
    pub duration: f32,

    // Dash.
    pub velocity_spike: Option<f32>,
    pub invincibility_time: Option<f32>,

    // Turbo.
    pub thrust_mult: Option<f32>,

    // SlowMo.
    pub time_scale: Option<f32>,
    pub player_speed_boost: Option<f32>,
    /// Multiplies the player's aim rotation speed while slow-mo is active.
    pub rot_speed_boost: Option<f32>,
}
