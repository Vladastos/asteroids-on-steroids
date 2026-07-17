//! Port of `GameConfig/Models/VfxConfig.cs`.

use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VfxConfig {
    // Dust burst (vaporised cells / tiny shards).
    pub dust_count: f32,
    pub dust_size: f32,
    pub dust_ttl: f32,
    pub dust_speed: f32,
    /// Cone half-angle as a fraction of π.
    pub dust_spread: f32,

    // Impact flash.
    pub flash_size: f32,
    pub flash_ttl: f32,

    // Bullet tracer.
    pub tracer_length: f32,
    pub tracer_width: f32,

    // Debris polygon chunks (shed by vaporised cells).
    pub debris_ttl: f32,
    pub debris_scatter: f32,

    // Impact sparks.
    pub spark_count: f32,
    pub spark_speed: f32,
    pub spark_ttl: f32,
    pub spark_size: f32,
    pub spark_spread: f32,

    // Hitstop (brief freeze on big events).
    pub hitstop_player_hit: f32,
    pub hitstop_grenade: f32,
    pub hitstop_big_fracture: f32,
    pub hitstop_big_area: f32,
    pub hitstop_max: f32,

    // Floating score popups.
    pub popup_min_value: f32,
    pub popup_flush_window: f32,
    pub popup_ttl: f32,
    pub popup_rise_speed: f32,
    pub popup_min_size: f32,
    pub popup_max_size: f32,
    pub popup_ref_value: f32,
}

impl Default for VfxConfig {
    fn default() -> Self {
        Self {
            dust_count: 14.0,
            dust_size: 2.6,
            dust_ttl: 0.70,
            dust_speed: 60.0,
            dust_spread: 0.50,
            flash_size: 22.0,
            flash_ttl: 0.12,
            tracer_length: 26.0,
            tracer_width: 2.0,
            debris_ttl: 0.80,
            debris_scatter: 40.0,
            spark_count: 9.0,
            spark_speed: 430.0,
            spark_ttl: 0.20,
            spark_size: 2.0,
            spark_spread: 0.42,
            hitstop_player_hit: 0.055,
            hitstop_grenade: 0.06,
            hitstop_big_fracture: 0.04,
            hitstop_big_area: 1400.0,
            hitstop_max: 0.13,
            popup_min_value: 50.0,
            popup_flush_window: 0.35,
            popup_ttl: 0.85,
            popup_rise_speed: 46.0,
            popup_min_size: 13.0,
            popup_max_size: 30.0,
            popup_ref_value: 600.0,
        }
    }
}
