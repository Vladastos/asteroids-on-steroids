namespace AsteroidsGame.Config;

public class WaveSystemConfig
{
    public int   BaseCellCap                { get; set; } = 300;
    public int   MaxCellCap                 { get; set; } = 2000;
    public int   CellCapGrowthAmount        { get; set; } = 30;
    public float GrowthIntervalSeconds      { get; set; } = 30f;
    public int   BaseBudget                 { get; set; } = 20;
    public int   BudgetGrowthPerInterval    { get; set; } = 5;
    public float TriggerThreshold           { get; set; } = 0.30f;
    public float GracePeriodSeconds         { get; set; } = 8.0f;
    public float HardTriggerIntervalSeconds { get; set; } = 30.0f;
    public float SpawnDelaySeconds          { get; set; } = 1.5f;
    public float SizeBiasStart              { get; set; } = -0.2f;
    public float SizeBiasEnd                { get; set; } = 0.6f;
    public float SizeBiasRampEnd            { get; set; } = 600.0f;
    public float MothershpSpawnTime         { get; set; } = 600.0f;

    public Dictionary<string, SpawnBiasEntry> SpawnBias { get; set; } = new();

    /// <summary>Scripted one-shot waves that fire at specific game times with their own weights,
    /// budget, cell cap, and banner — independent of the normal wave loop.</summary>
    public List<SpecialWaveConfig> SpecialWaves { get; set; } = new();

    /// <summary>Spawn pattern for normal waves (special waves may override with their own).</summary>
    public SpawnPatternConfig Pattern { get; set; } = new();
}

public class SpecialWaveConfig
{
    public float  TriggerTime { get; set; }                     // game seconds at which it fires
    public int    Budget      { get; set; } = 120;
    public int    CellCap     { get; set; } = 500;
    public float  SizeBias    { get; set; } = 0f;               // 0 = uniform sizing
    public string Banner      { get; set; } = "SPECIAL WAVE";
    public Dictionary<string, float> Weights { get; set; } = new();  // asteroid/alien key → absolute weight
    /// <summary>Per-wave spawn pattern; null = the wave system's default pattern.</summary>
    public SpawnPatternConfig? Pattern { get; set; }
}

/// <summary>How a wave's bodies are placed and aimed when they enter the map.</summary>
public class SpawnPatternConfig
{
    /// <summary>scattered (independent border spots — the classic) · burst (one tight cluster
    /// around an anchor on one side) · wall (spread along one side) · pincer (split across two
    /// opposite sides).</summary>
    public string Pattern { get; set; } = "scattered";
    /// <summary>inward (at the world centre) · atPlayer (at the player's position at RELEASE time)
    /// · random · fixed (FixedAngle).</summary>
    public string Direction { get; set; } = "inward";
    /// <summary>Aim angle in degrees when Direction == "fixed" (0 = +X, 90 = +Y/down).</summary>
    public float FixedAngle { get; set; } = 0f;
    /// <summary>Seconds the wave takes to trickle in (bodies released spread across the window).
    /// 0 = everything appears at once.</summary>
    public float SpawnDuration { get; set; } = 0f;
    /// <summary>burst: cluster radius (px) around the anchor point.</summary>
    public float BurstRadius { get; set; } = 420f;
    /// <summary>wall/pincer: fraction of the side's length the line occupies (0..1).</summary>
    public float Spread { get; set; } = 0.6f;
    /// <summary>Multiplier on each body's sampled entry speed.</summary>
    public float SpeedMult { get; set; } = 1f;
    /// <summary>Aim cone half-angle (radians) jittered around the pattern direction.</summary>
    public float AimJitter { get; set; } = 0.35f;
}

public class SpawnBiasEntry
{
    public float W0 { get; set; }
    public float W1 { get; set; }
    public float T0 { get; set; }
    public float T1 { get; set; }
}

public class VortexConfig
{
    public float Centripetal          { get; set; } = 0.05f;
    public float Tangential           { get; set; } = 0.02f;
    public float Deadzone             { get; set; } = 800f;
    public float CapFrames            { get; set; } = 8f;
    public float VariationCentripetal { get; set; } = 0.3f;
    public float VariationTangential  { get; set; } = 0.3f;

    // ── Moving centre (Lissajous orbit around the map centre) ──────────────────
    /// <summary>Orbit amplitude (px) along X / Y. Auto-clamped so the centre never comes within
    /// BorderMargin of any edge. 0 = stationary on that axis.</summary>
    public float MoveAmpX             { get; set; } = 0f;
    public float MoveAmpY             { get; set; } = 0f;
    /// <summary>Seconds per full oscillation along X / Y. Distinct values trace an open Lissajous path.</summary>
    public float MovePeriodX          { get; set; } = 40f;
    public float MovePeriodY          { get; set; } = 31f;
    /// <summary>Phase offset (radians) of the Y oscillation — shapes the Lissajous figure (default π/2).</summary>
    public float MovePhase            { get; set; } = 1.5707964f;
    /// <summary>Keep-out distance (px) from the map borders the centre must never cross.</summary>
    public float BorderMargin         { get; set; } = 700f;
}

public class WorldConfig
{
    public int   Width             { get; set; } = 5760;
    public int   Height            { get; set; } = 3240;
    public float CameraFollowSpeed { get; set; } = 4f;
}

/// <summary>
/// The map-border rim: keeps bodies in, shoves campers off the walls, and — past a grace
/// period — erodes whatever lingers there. Anti-camping enforcement (pairs with the vortex pull).
/// </summary>
public class BorderHazardConfig
{
    public bool  Enabled      { get; set; } = true;

    // ── Damp: cancel outward velocity near an edge so nothing leaves the map (was hard-coded) ──
    /// <summary>Distance (px) from an edge within which outward velocity is damped.</summary>
    public float DampZone     { get; set; } = 200f;
    /// <summary>Exponential damping rate applied to outward velocity in the damp zone.</summary>
    public float DampStrength { get; set; } = 20f;

    // ── Push: an inward shove that grows toward the edge (nudges campers inward) ──
    /// <summary>Distance (px) from an edge within which the inward push applies.</summary>
    public float PushZone     { get; set; } = 420f;
    /// <summary>Inward acceleration (px/s²) at the very edge, fading linearly to 0 at PushZone.</summary>
    public float PushStrength { get; set; } = 1200f;

    // ── Erosion: after grace, the storm rips the most-exposed cell every Tick, ramping over time ──
    /// <summary>Distance (px) from an edge within which erosion exposure accumulates.</summary>
    public float HazardZone   { get; set; } = 340f;
    /// <summary>Seconds a body may sit in the hazard rim before erosion begins.</summary>
    public float Grace        { get; set; } = 2.0f;
    /// <summary>Seconds between erosion hits once past the grace period.</summary>
    public float Tick         { get; set; } = 0.4f;
    /// <summary>Synthetic impactor mass of the first erosion hit (fracture energy scales with it).</summary>
    public float BaseMass     { get; set; } = 8f;
    /// <summary>Impactor-mass growth per extra second camped past grace (energy ramp).</summary>
    public float Ramp         { get; set; } = 0.5f;
    /// <summary>Normal speed of the synthetic erosion impact (with BaseMass sets the base energy).</summary>
    public float ImpactSpeed  { get; set; } = 1000f;
    /// <summary>Exposure recovery rate (× dt) once a body leaves the hazard rim.</summary>
    public float DecayRate    { get; set; } = 0.5f;
}
