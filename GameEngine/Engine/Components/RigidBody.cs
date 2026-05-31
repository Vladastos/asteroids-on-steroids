using System.Numerics;

namespace AsteroidsEngine.Engine.Components;

/// <summary>
/// Physics properties. Consumed by PhysicsSystem.
/// Entities without a RigidBody are not affected by forces or drag.
/// </summary>
public struct RigidBody
{
    public float   Mass;              // kg; used for impulse response
    public float   LinearDrag;        // 0 = no drag; higher = stops faster (decay rate, s⁻¹)
    public float   AngularDrag;       // same, for rotation
    public float   Inertia;           // moment of inertia (kg·px²); computed at spawn via PolygonUtils.ComputeInertia
    public Vector2 AccumulatedForce;  // reset each frame after integration
    public float   AccumulatedTorque; // N·px; reset each frame after integration

    /// <summary>
    /// Coefficient of restitution [0 = perfectly inelastic, 1 = perfectly elastic].
    /// CollisionSystem uses min(eA, eB) for each colliding pair.
    /// Default 0 (no bounce) — set explicitly at entity spawn.
    /// </summary>
    public float Restitution;
}
