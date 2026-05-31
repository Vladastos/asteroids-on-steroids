using System.Numerics;
using AsteroidsEngine.Engine.Components;
using AsteroidsEngine.Engine.Core;

namespace AsteroidsEngine.Engine.Destruction;

/// <summary>Tunable constants of the energy model (docs/destruction_engine_spec.md §4.4–4.5).</summary>
public struct FractureSettings
{
    public float SpinEnergyFraction;   // fraction of ½Iω² treated as fracture pre-stress energy
    public float MomentumTransfer;     // fraction of bullet momentum imparted to fragments

    public static FractureSettings Default => new()
    {
        SpinEnergyFraction = 0.35f,
        MomentumTransfer   = 0.55f,
    };
}

/// <summary>
/// ECS entry point for fracturing (the thin-engine boundary, spec §5). Given a hit
/// on a FracturableBody entity it reads the body's components, computes the impact
/// energy (reduced-mass + spin pre-stress), runs the FractureSimulator, and:
///   • sub-threshold → accumulates the energy on the body in place, returns false;
///   • at/above threshold → returns true with a FractureResult.
/// The engine spawns no entities — the caller wires the result (spawn fragments,
/// destroy the original) and may raise its own gameplay events.
/// </summary>
public static class FractureService
{
    public static bool TryFracture(
        World world, Entity body, int struckCell,
        Vector2 impactPoint, Vector2 impactDir, Vector2 impactorVelocity, float impactorMass,
        float directionality, Random rng, out FractureResult result)
        => TryFracture(world, body, struckCell, impactPoint, impactDir, impactorVelocity,
                       impactorMass, directionality, FractureSettings.Default, rng, out result);

    public static bool TryFracture(
        World world, Entity body, int struckCell,
        Vector2 impactPoint, Vector2 impactDir, Vector2 impactorVelocity, float impactorMass,
        float directionality, in FractureSettings settings, Random rng, out FractureResult result)
    {
        result = default;

        if (!world.IsAlive(body)) return false;
        if (!world.HasComponent<FracturableBody>(body) ||
            !world.HasComponent<Transform>(body) ||
            !world.HasComponent<RigidBody>(body)) return false;

        ref var fb = ref world.GetComponent<FracturableBody>(body);
        ref var t  = ref world.GetComponent<Transform>(body);
        ref var rb = ref world.GetComponent<RigidBody>(body);
        if (fb.Cells.Length == 0) return false;

        Vector2 bodyLinear  = Vector2.Zero;
        float   bodyAngular = 0f;
        if (world.HasComponent<Velocity>(body))
        {
            ref var v = ref world.GetComponent<Velocity>(body);
            bodyLinear  = v.Linear;
            bodyAngular = v.Angular;
        }

        // Resolve the struck cell (raycast PartIndex; fall back to nearest).
        int cell = struckCell;
        if (cell < 0 || cell >= fb.Cells.Length)
            cell = NearestCell(fb, t.Position, t.Rotation, impactPoint);

        // --- Energy model ---
        Vector2 dir   = impactDir.LengthSquared() > 1e-8f ? Vector2.Normalize(impactDir) : Vector2.UnitX;
        float   mBody = rb.Mass;
        float   vRelN = MathF.Abs(Vector2.Dot(impactorVelocity - bodyLinear, dir));
        float   mRed  = (impactorMass + mBody) > 0f ? impactorMass * mBody / (impactorMass + mBody) : impactorMass;
        float   eImpact = 0.5f * mRed * vRelN * vRelN;
        float   eSpin   = settings.SpinEnergyFraction * 0.5f * rb.Inertia * bodyAngular * bodyAngular;

        Vector2 kick = mBody > 1e-6f
            ? dir * (impactorVelocity.Length() * impactorMass / mBody * settings.MomentumTransfer)
            : Vector2.Zero;

        var input = new FractureInput
        {
            StruckCell       = cell,
            EnergyTotal      = eImpact + eSpin,
            ImpactPointWorld = impactPoint,
            ImpactDir        = dir,
            Directionality   = directionality,
            SpinOmega        = bodyAngular,
            MomentumKick     = kick,
            BodyPosition     = t.Position,
            BodyRotation     = t.Rotation,
            BodyLinear       = bodyLinear,
            BodyAngular      = bodyAngular,
            BodyMass         = mBody,
        };

        result = FractureSimulator.Simulate(fb, input, rng);

        if (!result.Fractured)
        {
            fb.State.AbsorbedEnergy = result.AbsorbedEnergy;   // accumulate sub-threshold damage in place
            return false;
        }
        return true;
    }

    private static int NearestCell(in FracturableBody fb, Vector2 pos, float rot, Vector2 worldPoint)
    {
        float cos = MathF.Cos(rot), sin = MathF.Sin(rot);
        Vector2 d = worldPoint - pos;
        Vector2 local = new(d.X * cos + d.Y * sin, -d.X * sin + d.Y * cos);   // un-rotate into body space

        int   best   = 0;
        float bestSq = float.MaxValue;
        for (int i = 0; i < fb.Cells.Length; i++)
        {
            float sq = (fb.Cells[i].Centroid - local).LengthSquared();
            if (sq < bestSq) { bestSq = sq; best = i; }
        }
        return best;
    }
}
