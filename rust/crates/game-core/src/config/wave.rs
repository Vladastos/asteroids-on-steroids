//! Port of `GameConfig/Models/WaveDefinition.cs`.

use serde::Deserialize;
use std::collections::HashMap;

/// One wave entry. `type == "budget"`: game picks spawns from `spawns` weights
/// until `budget` is spent (unit = total cell count of spawned entity).
/// `type == "explicit"`: spawns exactly the list in `asteroids`. Both types use
/// `spawn_pattern` and `modifiers`.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WaveDefinition {
    pub wave: i32,
    /// "budget" or "explicit".
    #[serde(rename = "type")]
    pub kind: String,

    // Budget wave.
    /// How many asteroids to spawn (budget type). 0 = fall back to the global
    /// tunable. Explicit waves use per-group `count` fields.
    pub asteroid_count: i32,
    /// Total cell-count budget for this wave (budget type only).
    pub budget: f32,
    /// Entity-type key → relative spawn weight (budget type only).
    pub spawns: Option<HashMap<String, f32>>,

    // Explicit wave.
    pub asteroids: Option<Vec<ExplicitSpawn>>,

    /// Size distribution bias: -1 = prefer small, 0 = uniform, +1 = prefer large.
    pub size_bias: f32,

    // Common.
    /// "burst" = all at once, "rapid" = one every `rapid_interval` s,
    /// "staggered" = one every 0.5-1.5s (randomised).
    pub spawn_pattern: String,
    pub rapid_interval: f32,
    /// Named modifier tags applied to every asteroid in this wave.
    pub modifiers: Vec<String>,
    pub boss: bool,
}

impl Default for WaveDefinition {
    fn default() -> Self {
        Self {
            wave: 0,
            kind: "budget".into(),
            asteroid_count: 0,
            budget: 0.0,
            spawns: None,
            asteroids: None,
            size_bias: 0.0,
            spawn_pattern: "burst".into(),
            rapid_interval: 0.4,
            modifiers: Vec::new(),
            boss: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ExplicitSpawn {
    #[serde(rename = "type")]
    pub kind: String,
    pub count: i32,
    /// Seconds after the wave trigger before this group spawns.
    pub spawn_delay: f32,
}

impl Default for ExplicitSpawn {
    fn default() -> Self {
        Self {
            kind: String::new(),
            count: 1,
            spawn_delay: 0.0,
        }
    }
}
