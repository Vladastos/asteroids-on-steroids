//! Global, model-shaping fracture scalars — port of `FractureTuning` in
//! `FractureKernel.cs`. In C# these are mutable statics set from config; here they
//! are `const` defaults. When you wire config (Phase 1d), lift them into a
//! `FractureTuning` struct threaded through the calls instead of `const`.

pub const ENERGY_SCALE: f32 = 0.0001; // physical ½·m·v² → fracture-energy units
pub const REACH_MIN: f32 = 0.1; // transmit fraction at brittleness 0
pub const REACH_MAX: f32 = 0.96; // transmit fraction at brittleness 1
pub const VAPOR_EFF: f32 = 0.4; // blast-wave penetration: surplus continued vs lost to heat
pub const BREAK_PERP: f32 = 1.0; // 0 = break ALONG flow … 1 = PERPENDICULAR
pub const ALIGN_EXPONENT: f32 = 1.6; // directional cone sharpness
pub const SPIN_CAP: f32 = 4.0; // max spin stress multiplier
pub const FLING_SCALE: f32 = 140.0; // fling energy → fragment speed
pub const FRAGMENT_SPEED_MAX: f32 = 600.0; // clamp on fling speed (px/s)
pub const TUMBLE_SCALE: f32 = 220.0; // fling-asymmetry → fragment spin gain
pub const FRAGMENT_SPIN_MAX: f32 = 3.5; // clamp on fragment spin (rad/s)
pub const SPIN_PROFILE_BASE: f32 = 0.3; // spin pre-stress at centre; rises to 1.0 at rim
pub const SPLIT_STRESS_INHERIT: f32 = 1.0; // fraction of Damage/Stress fragments keep on split

// Impact-velocity → crack-speed coupling.
pub const CRACK_SPEED_REF_VELOCITY: f32 = 600.0;
pub const CRACK_SPEED_VEL_EXPONENT: f32 = 0.5;
pub const CRACK_SPEED_MULT_MIN: f32 = 0.25;
pub const CRACK_SPEED_MULT_MAX: f32 = 4.0;

/// Crack-speed multiplier for a hit at `normal_speed`. Port of `CrackSpeedFactor`.
pub fn crack_speed_factor(normal_speed: f32) -> f32 {
    if normal_speed <= 0.0 || CRACK_SPEED_VEL_EXPONENT <= 0.0 {
        return 1.0;
    }
    let g = (normal_speed / CRACK_SPEED_REF_VELOCITY.max(1.0)).powf(CRACK_SPEED_VEL_EXPONENT);
    g.clamp(CRACK_SPEED_MULT_MIN, CRACK_SPEED_MULT_MAX)
}

#[inline]
pub(crate) fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
