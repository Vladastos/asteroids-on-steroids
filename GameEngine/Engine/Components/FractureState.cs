namespace AsteroidsEngine.Engine.Components;

/// <summary>
/// Mutable per-entity fracture runtime state.
/// Kept separate from FractureProperties so presets remain static/shareable.
/// </summary>
public struct FractureState
{
    /// <summary>
    /// Accumulated impact energy from sub-threshold hits.
    /// Combined with the current hit's energy before the fracture threshold check.
    /// Reset to zero when the entity fractures.
    /// </summary>
    public float AbsorbedEnergy;

    /// <summary>
    /// Pre-scored fault directions (radians) generated at spawn.
    /// Cut planes in Split() snap toward these angles, making each asteroid
    /// crack along characteristic weak lines.
    /// Decremented by one fault per generation of sub-fragments (minimum 0).
    /// </summary>
    public float[] FaultAngles;
}
