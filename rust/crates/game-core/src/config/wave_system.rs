//! Port of `GameConfig/Models/WaveSystemConfig.cs`.

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WaveSystemConfig {
    pub base_cell_cap: i32,
    pub max_cell_cap: i32,
    pub cell_cap_growth_amount: i32,
    pub growth_interval_seconds: f32,
    pub base_budget: i32,
    pub budget_growth_per_interval: i32,
    pub trigger_threshold: f32,
    pub grace_period_seconds: f32,
    pub hard_trigger_interval_seconds: f32,
    pub spawn_delay_seconds: f32,
    pub size_bias_start: f32,
    pub size_bias_end: f32,
    pub size_bias_ramp_end: f32,
    pub mothersph_spawn_time: f32,

    pub spawn_bias: HashMap<String, SpawnBiasEntry>,
    /// Scripted one-shot waves independent of the normal wave loop.
    pub special_waves: Vec<SpecialWaveConfig>,
    /// Spawn pattern for normal waves (special waves may override with their own).
    pub pattern: SpawnPatternConfig,
    /// Anti-camping response: lingering near the border builds a timer that
    /// sends hunter waves at the player from their nearest side.
    pub camping_response: CampingResponseConfig,
}

impl Default for WaveSystemConfig {
    fn default() -> Self {
        Self {
            base_cell_cap: 300,
            max_cell_cap: 2000,
            cell_cap_growth_amount: 30,
            growth_interval_seconds: 30.0,
            base_budget: 20,
            budget_growth_per_interval: 5,
            trigger_threshold: 0.30,
            grace_period_seconds: 8.0,
            hard_trigger_interval_seconds: 30.0,
            spawn_delay_seconds: 1.5,
            size_bias_start: -0.2,
            size_bias_end: 0.6,
            size_bias_ramp_end: 600.0,
            mothersph_spawn_time: 600.0,
            spawn_bias: HashMap::new(),
            special_waves: Vec::new(),
            pattern: SpawnPatternConfig::default(),
            camping_response: CampingResponseConfig::default(),
        }
    }
}

/// Camping is tracked as time spent inside the border band. The timer decays
/// when the player leaves — it does not reset. At `trigger_seconds` a hunter
/// wave fires from the player's nearest side, then again every `repeat_seconds`.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CampingResponseConfig {
    pub enabled: bool,
    /// Band beyond the erosion rim that still counts as camping (px).
    pub zone_depth: f32,
    /// Corner reach as a multiple of the edge band.
    pub corner_scale: f32,
    pub trigger_seconds: f32,
    /// Timer decay per second while outside the zone.
    pub decay_rate: f32,
    pub repeat_seconds: f32,
    pub budget: i32,
    pub cell_cap: i32,
    pub size_bias: f32,
    pub banner: String,
    /// Full-screen grey filter alpha at full pressure. 0 disables it.
    pub tint_max_alpha: f32,
    pub weights: HashMap<String, f32>,
    pub pattern: SpawnPatternConfig,
}

impl Default for CampingResponseConfig {
    fn default() -> Self {
        let mut weights = HashMap::new();
        weights.insert("drone".to_string(), 1.0);
        Self {
            enabled: true,
            zone_depth: 200.0,
            corner_scale: 1.8,
            trigger_seconds: 20.0,
            decay_rate: 0.5,
            repeat_seconds: 12.0,
            budget: 90,
            cell_cap: 400,
            size_bias: 0.0,
            banner: "HUNTERS INBOUND".into(),
            tint_max_alpha: 70.0,
            weights,
            pattern: SpawnPatternConfig {
                pattern: "burst".into(),
                direction: "atPlayer".into(),
                side: "nearPlayer".into(),
                spawn_duration: 2.0,
                ..SpawnPatternConfig::default()
            },
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SpecialWaveConfig {
    /// Game seconds at which it fires.
    pub trigger_time: f32,
    pub budget: i32,
    pub cell_cap: i32,
    /// 0 = uniform sizing.
    pub size_bias: f32,
    pub banner: String,
    /// asteroid/alien key → absolute weight.
    pub weights: HashMap<String, f32>,
    /// Per-wave spawn pattern; `None` = the wave system's default pattern.
    pub pattern: Option<SpawnPatternConfig>,
}

impl Default for SpecialWaveConfig {
    fn default() -> Self {
        Self {
            trigger_time: 0.0,
            budget: 120,
            cell_cap: 500,
            size_bias: 0.0,
            banner: "SPECIAL WAVE".into(),
            weights: HashMap::new(),
            pattern: None,
        }
    }
}

/// How a wave's bodies are placed and aimed when they enter the map.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SpawnPatternConfig {
    /// scattered · burst · wall · pincer.
    pub pattern: String,
    /// inward · atPlayer · random · fixed (`fixed_angle`).
    pub direction: String,
    /// Which border the wave enters from: random · nearPlayer.
    pub side: String,
    /// Aim angle in degrees when `direction == "fixed"` (0 = +X, 90 = +Y/down).
    pub fixed_angle: f32,
    /// Seconds the wave takes to trickle in. 0 = everything at once.
    pub spawn_duration: f32,
    /// burst: cluster radius (px) around the anchor point.
    pub burst_radius: f32,
    /// wall/pincer: fraction of the side's length the line occupies (0..1).
    pub spread: f32,
    pub speed_mult: f32,
    /// Aim cone half-angle (radians) jittered around the pattern direction.
    pub aim_jitter: f32,
}

impl Default for SpawnPatternConfig {
    fn default() -> Self {
        Self {
            pattern: "scattered".into(),
            direction: "inward".into(),
            side: "random".into(),
            fixed_angle: 0.0,
            spawn_duration: 0.0,
            burst_radius: 420.0,
            spread: 0.6,
            speed_mult: 1.0,
            aim_jitter: 0.35,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SpawnBiasEntry {
    pub w0: f32,
    pub w1: f32,
    pub t0: f32,
    pub t1: f32,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VortexConfig {
    pub centripetal: f32,
    pub tangential: f32,
    pub deadzone: f32,
    pub cap_frames: f32,
    pub variation_centripetal: f32,
    pub variation_tangential: f32,

    // Moving centre (Lissajous orbit around the map centre).
    pub move_amp_x: f32,
    pub move_amp_y: f32,
    pub move_period_x: f32,
    pub move_period_y: f32,
    /// Phase offset (radians) of the Y oscillation.
    pub move_phase: f32,
    /// Keep-out distance (px) from the map borders the centre must never cross.
    pub border_margin: f32,
}

impl Default for VortexConfig {
    fn default() -> Self {
        Self {
            centripetal: 0.05,
            tangential: 0.02,
            deadzone: 800.0,
            cap_frames: 8.0,
            variation_centripetal: 0.3,
            variation_tangential: 0.3,
            move_amp_x: 0.0,
            move_amp_y: 0.0,
            move_period_x: 40.0,
            move_period_y: 31.0,
            move_phase: std::f32::consts::FRAC_PI_2,
            border_margin: 700.0,
        }
    }
}

/// Vortex visualisation: sporadic wind-gust motes advected along the real force
/// field, drawn as fading streaks, plus an optional screen-space swirl warp.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VortexFxConfig {
    pub enabled: bool,
    pub gust_interval: f32,
    pub gust_jitter: f32,
    pub motes_per_gust: i32,
    pub max_motes: i32,
    pub ttl: f32,
    pub ttl_jitter: f32,
    /// Radius (world px) of the disc around the eye where motes spawn.
    pub max_radius: f32,
    /// Streak tail length, in seconds of the mote's velocity.
    pub streak_seconds: f32,
    pub speed_scale: f32,
    pub color: [f32; 3],

    // Screen-space swirl warp centred on the eye.
    pub warp_enabled: bool,
    pub warp_radius: f32,
    /// Peak twist (radians) at the eye, falling to 0 at `warp_radius`.
    pub warp_strength: f32,
    pub warp_grid: i32,
}

impl Default for VortexFxConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            gust_interval: 0.5,
            gust_jitter: 0.6,
            motes_per_gust: 14,
            max_motes: 400,
            ttl: 2.2,
            ttl_jitter: 0.4,
            max_radius: 1600.0,
            streak_seconds: 0.09,
            speed_scale: 1.0,
            color: [150.0, 130.0, 240.0],
            warp_enabled: true,
            warp_radius: 620.0,
            warp_strength: 0.6,
            warp_grid: 20,
        }
    }
}

/// Playable field size. The camera clamps to it and the border hazard encloses it.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorldConfig {
    pub width: i32,
    pub height: i32,
    pub camera_follow_speed: f32,
    /// Width (px) of the spawn ring outside the playable field. Wave bodies
    /// spawn there and drift in — guaranteed off-screen.
    pub spawn_margin: f32,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            width: 5760,
            height: 3240,
            camera_follow_speed: 4.0,
            spawn_margin: 500.0,
        }
    }
}

/// The map-border rim: keeps bodies in, shoves campers off the walls, and —
/// past a grace period — erodes whatever lingers there.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BorderHazardConfig {
    pub enabled: bool,

    // Damp: cancel outward velocity near an edge.
    pub damp_zone: f32,
    pub damp_strength: f32,

    // Push: inward shove that grows toward the edge.
    pub push_zone: f32,
    pub push_strength: f32,

    // Erosion: after grace, the storm rips the most-exposed cell every tick.
    pub hazard_zone: f32,
    pub grace: f32,
    pub tick: f32,
    /// Synthetic impactor mass of the first erosion hit.
    pub base_mass: f32,
    /// Impactor-mass growth per extra second camped past grace.
    pub ramp: f32,
    pub impact_speed: f32,
    pub decay_rate: f32,

    // Visuals.
    pub tint_max_alpha: f32,
    pub warp_enabled: bool,
    pub warp_strength: f32,
    pub warp_freq: f32,
    pub warp_speed: f32,
    pub warp_grid: i32,
}

impl Default for BorderHazardConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            damp_zone: 200.0,
            damp_strength: 20.0,
            push_zone: 420.0,
            push_strength: 1200.0,
            hazard_zone: 340.0,
            grace: 2.0,
            tick: 0.4,
            base_mass: 8.0,
            ramp: 0.5,
            impact_speed: 1000.0,
            decay_rate: 0.5,
            tint_max_alpha: 120.0,
            warp_enabled: true,
            warp_strength: 7.0,
            warp_freq: 0.03,
            warp_speed: 2.2,
            warp_grid: 24,
        }
    }
}
