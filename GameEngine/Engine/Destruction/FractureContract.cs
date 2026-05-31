using System.Numerics;

namespace AsteroidsEngine.Engine.Destruction;

/// <summary>
/// Inputs to one fracture evaluation. The FractureSystem computes the energy terms
/// from the collision/velocities; the simulator does the geometry + physics. Angles
/// in radians; directions are unit vectors in world space.
/// </summary>
public struct FractureInput
{
    public int     StruckCell;        // index of the cell hit
    public float   EnergyTotal;       // E_impact + E_spin (already computed)
    public Vector2 ImpactPointWorld;  // exact surface hit point
    public Vector2 ImpactDir;         // shot direction (steers the directional crack cone)
    public float   Directionality;    // 0 = isotropic splash … 1 = forward channel
    public float   SpinOmega;         // body angular velocity (centrifugal pre-stress)
    public Vector2 MomentumKick;      // bullet momentum added to all fragments' linear velocity

    public Vector2 BodyPosition;      // Transform.Position (world centroid)
    public float   BodyRotation;
    public Vector2 BodyLinear;
    public float   BodyAngular;
    public float   BodyMass;          // current RigidBody.Mass
}

/// <summary>One resulting body from a fracture: a connected component of cells.</summary>
public struct FragmentSpec
{
    public FracturableBody Body;       // re-centred cells + internal bonds, ready to spawn
    public Vector2 WorldCentroid;      // Transform.Position for the new entity
    public float   Rotation;           // Transform.Rotation (= parent's; cells are parent-local un-rotated)
    public Vector2 Linear;
    public float   Angular;
    public float   Mass;
    public float   Inertia;
    public float   Area;
    public bool    IsDebris;           // single tiny cell → visual particle, not a physics body
}

/// <summary>
/// Output of the fracture simulator. The engine computes it and emits it; the game
/// wires entities from it (thin-engine contract, spec §5).
/// </summary>
public struct FractureResult
{
    public bool           Fractured;       // false = sub-threshold (absorb, no split)
    public float          AbsorbedEnergy;  // updated accumulator (valid when !Fractured)
    public FragmentSpec[] Fragments;       // all resulting bodies (largest = survivor)
    public Vector2        ImpactPointWorld;
    public float          EnergySurface;   // budget spent breaking bonds
    public float          EnergyKinetic;   // energy given to fragments as fling
}
