namespace AsteroidsEngine.Engine.Components;

/// <summary>
/// Immutable material description for fracturable entities.
/// Use the static presets or construct custom values; share freely across entities.
/// Runtime mutable state lives in FractureState.
/// </summary>
public struct FractureProperties
{
    /// <summary>[0 = fully ductile, 1 = fully brittle/glass-like]
    /// Controls cut count, angular spread of cuts, and blast radius size.
    /// Ductile: few concentrated cuts near impact; far side survives intact.
    /// Brittle: many evenly distributed cuts; uniform shattering.
    /// </summary>
    public float Brittleness;

    /// <summary>
    /// Minimum impact energy per unit mass to begin fracturing.
    /// threshold = Toughness × RigidBody.Mass
    /// Below threshold the hit accumulates in FractureState.AbsorbedEnergy.
    /// </summary>
    public float Toughness;

    /// <summary>Number of pre-scored fault angles baked into FractureState at spawn.</summary>
    public int FaultCount;

    /// <summary>
    /// Fragment area (px²) below which a piece is spawned as fading debris
    /// rather than a live collidable fragment.
    /// </summary>
    public float MinFragmentArea;

    // ---- Presets ----
    // Calibrated so standard bullet (KE ≈ 33 600) on reference asteroid (mass ≈ 4)
    // gives severity ≈ 1 for Rock.

    public static readonly FractureProperties Glass = new()
    {
        Brittleness = 1.00f, Toughness =    840f, FaultCount = 0, MinFragmentArea =  40f,
    };
    public static readonly FractureProperties Ice = new()
    {
        Brittleness = 0.80f, Toughness =  2_100f, FaultCount = 7, MinFragmentArea =  80f,
    };
    public static readonly FractureProperties Rock = new()
    {
        Brittleness = 0.60f, Toughness =  8_400f, FaultCount = 4, MinFragmentArea = 180f,
    };
    public static readonly FractureProperties Metal = new()
    {
        Brittleness = 0.15f, Toughness = 84_000f, FaultCount = 2, MinFragmentArea = 400f,
    };
}
