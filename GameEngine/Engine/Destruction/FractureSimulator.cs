using System.Numerics;
using AsteroidsEngine.Engine.Collision;
using AsteroidsEngine.Engine.Components;

namespace AsteroidsEngine.Engine.Destruction;

/// <summary>
/// The pure-logic core of the destruction model (docs/destruction_engine_spec.md
/// §4.4–4.8). Given a body, the struck cell and the impact energy, it:
///   1. checks the fracture threshold (sub-threshold → absorb, no split),
///   2. splits the available energy into a surface budget + a kinetic share,
///   3. propagates a crack from the struck cell, spending the budget breaking bonds
///      (brittleness = reach, directionality = which bonds, spin = pre-stress),
///   4. finds connected components over the surviving bonds,
///   5. builds a FragmentSpec per component (re-centred geometry, mass, inertia,
///      velocity from parent motion + kinetic fling).
/// No ECS dependency — the FractureSystem feeds it and wires the result to entities.
/// </summary>
public static class FractureSimulator
{
    private const float Transmission0 = 0.18f;  // brittleness 0 → short reach
    private const float Transmission1 = 0.96f;  // brittleness 1 → long reach
    private const float MinEffStrengthFrac = 0.02f;

    public static FractureResult Simulate(in FracturableBody body, in FractureInput input, Random rng)
    {
        Cell[] cells = body.Cells;
        Bond[] bonds = body.Bonds;
        FractureProperties mat = body.Material;

        var budget = ComputeBudget(body, input.StruckCell, input.BodyMass, input.EnergyTotal);
        if (!budget.Fractured)
            return new FractureResult
            {
                Fractured = false,
                AbsorbedEnergy = budget.Absorbed,
                ImpactPointWorld = input.ImpactPointWorld,
            };

        float totalArea = budget.TotalArea;
        float eSurface = budget.Surface;
        float eKinetic = budget.Kinetic;

        var eff = new float[bonds.Length];
        ComputeEffStrengths(cells, bonds, input.SpinOmega, mat.SpinPreStress, eff);

        var adj = BuildAdjacency(cells.Length, bonds);
        var broken = new bool[bonds.Length];
        var energy = new float[cells.Length];
        Propagate(cells, bonds, adj, eff, broken, energy,
                  input.StruckCell, input.EnergyTotal, eSurface,
                  RotateInv(input.ImpactDir, input.BodyRotation),
                  input.Directionality, mat.Brittleness);

        // Cells that absorb enough crack energy are PULVERISED → vaporise to debris.
        // This carves the impact crater and opens the surface, so subsurface fragments
        // aren't left trapped under intact cells.
        var pulverized = new bool[cells.Length];
        float blast = Math.Clamp(input.BlastFraction, 0f, 1f);
        if (blast > 0f)
        {
            // Vaporisation crater. The struck cell holds the full impact energy and the
            // energy decays outward, so a threshold of (1-blast)·E selects the hottest
            // cells around the hit: blast→0 vaporises nothing (pure fragmentation),
            // blast→1 vaporises everything the crack reached.
            float blastThresh = (1f - blast) * input.EnergyTotal;
            for (int i = 0; i < cells.Length; i++)
                if (energy[i] > 0f && energy[i] > blastThresh) pulverized[i] = true;
        }

        int[] comp = ConnectedComponents(cells.Length, bonds, broken, pulverized, out int compCount);
        var fragments = BuildFragments(body, input, comp, compCount, broken, pulverized, totalArea, rng);

        return new FractureResult
        {
            Fractured = true,
            AbsorbedEnergy = 0f,
            Fragments = fragments,
            ImpactPointWorld = input.ImpactPointWorld,
            EnergySurface = eSurface,
            EnergyKinetic = eKinetic,
        };
    }

    // -------------------------------------------------------------------------
    // Shared building blocks (used by both single-frame Simulate and the
    // multi-frame FractureCrackSystem so both paths use identical physics).
    // -------------------------------------------------------------------------

    /// <summary>Energy split after the fracture threshold: a surface (crack) budget and
    /// a kinetic (fling) share. Fractured == false means the hit was sub-threshold.</summary>
    public readonly struct FractureBudget
    {
        public readonly bool Fractured;
        public readonly float Surface, Kinetic, Absorbed, TotalArea;
        public FractureBudget(bool fractured, float surface, float kinetic, float absorbed, float totalArea)
        { Fractured = fractured; Surface = surface; Kinetic = kinetic; Absorbed = absorbed; TotalArea = totalArea; }
    }

    public static FractureBudget ComputeBudget(in FracturableBody body, int struckCell, float bodyMass, float energyTotal)
    {
        Cell[] cells = body.Cells;
        FractureProperties mat = body.Material;

        float totalArea = 0f;
        foreach (var c in cells) totalArea += c.Area;

        float struckMass = bodyMass * (cells[struckCell].Area / totalArea);
        float threshold = mat.Toughness * struckMass;
        float combined = energyTotal + body.State.AbsorbedEnergy;
        if (combined < threshold)
            return new FractureBudget(false, 0f, 0f, body.State.AbsorbedEnergy + energyTotal, totalArea);

        float eAvail = combined - threshold;
        float kFrac = Lerp(mat.KineticFraction, mat.KineticFraction * 0.3f, mat.Brittleness);
        // Only a fraction of the available energy creates fracture surface (rest → heat/sound).
        float surfEff = mat.SurfaceEfficiency > 0f ? mat.SurfaceEfficiency : 1f;
        return new FractureBudget(true, surfEff * (1f - kFrac) * eAvail, kFrac * eAvail, 0f, totalArea);
    }

    /// <summary>Snapshot the bond graph for a live fracture: effective bond strengths
    /// (with spin pre-stress) and the per-cell adjacency.</summary>
    public static (float[] eff, List<int>[] adj) PrepareGraph(in FracturableBody body, float spinOmega)
    {
        var eff = new float[body.Bonds.Length];
        ComputeEffStrengths(body.Cells, body.Bonds, spinOmega, body.Material.SpinPreStress, eff);
        var adj = BuildAdjacency(body.Cells.Length, body.Bonds);
        return (eff, adj);
    }

    /// <summary>Crack transmission (reach) for a brittleness, for seeding a CrackFront.</summary>
    public static float TransmissionFor(float brittleness) => Lerp(Transmission0, Transmission1, brittleness);

    /// <summary>Finalise a multi-frame fracture: connected components over the accumulated
    /// broken/pulverised state → fragment specs. Pulverised cells are NOT re-emitted as
    /// debris (the live system already vaporised them to dust as it propagated).</summary>
    public static FragmentSpec[] BuildResult(in FracturableBody body, in FractureInput input,
        bool[] broken, bool[] pulverized, Random rng)
    {
        float totalArea = 0f;
        foreach (var c in body.Cells) totalArea += c.Area;
        int[] comp = ConnectedComponents(body.Cells.Length, body.Bonds, broken, pulverized, out int compCount);
        return BuildFragments(body, input, comp, compCount, broken, pulverized, totalArea, rng,
                              includePulverizedDebris: false);
    }

    // -------------------------------------------------------------------------
    // Spin pre-stress: weaken tangentially-oriented bonds, growing outward.
    // -------------------------------------------------------------------------
    private static void ComputeEffStrengths(Cell[] cells, Bond[] bonds, float omega, float spinCoeff, float[] eff)
    {
        if (bonds.Length == 0) return;

        float avg = 0f;
        foreach (var b in bonds) avg += b.Strength;
        avg /= bonds.Length;

        float rmax = 1e-3f;
        foreach (var c in cells) rmax = MathF.Max(rmax, c.Centroid.Length());

        float w2 = omega * omega;
        for (int k = 0; k < bonds.Length; k++)
        {
            Bond b = bonds[k];
            Vector2 m = (cells[b.A].Centroid + cells[b.B].Centroid) * 0.5f;   // CoM = origin
            float r = m.Length();
            if (r < 1e-3f) { eff[k] = b.Strength; continue; }

            Vector2 rad = m / r;
            Vector2 tan = new(-rad.Y, rad.X);
            Vector2 dir = cells[b.B].Centroid - cells[b.A].Centroid;
            float dl = dir.Length();
            if (dl > 1e-6f) dir /= dl;

            float tangentiality = MathF.Abs(Vector2.Dot(dir, tan));
            float profile = 0.3f + 0.7f * (r / rmax);
            float preStress = spinCoeff * avg * w2 * profile * tangentiality;
            eff[k] = MathF.Max(b.Strength * MinEffStrengthFrac, b.Strength - preStress);
        }
    }

    private static List<int>[] BuildAdjacency(int n, Bond[] bonds)
    {
        var adj = new List<int>[n];
        for (int i = 0; i < n; i++) adj[i] = new List<int>();
        for (int k = 0; k < bonds.Length; k++) { adj[bonds[k].A].Add(k); adj[bonds[k].B].Add(k); }
        return adj;
    }

    // -------------------------------------------------------------------------
    // Crack propagation — descending-energy flood spending a surface budget.
    // -------------------------------------------------------------------------
    private static void Propagate(
        Cell[] cells, Bond[] bonds, List<int>[] adj, float[] eff, bool[] broken, float[] energy,
        int struck, float startEnergy, float budget,
        Vector2 impactDirLocal, float directionality, float brittleness)
    {
        var front = CrackFront.Seed(energy, struck, startEnergy, budget,
                                    impactDirLocal, directionality, TransmissionFor(brittleness),
                                    float.PositiveInfinity);   // pulverisation handled separately below
        while (front.Active)
            FractureKernel.StepFront(front, cells, bonds, adj, eff, broken);
    }

    private static int[] ConnectedComponents(int n, Bond[] bonds, bool[] broken, bool[] pulverized, out int count)
    {
        var parent = new int[n];
        for (int i = 0; i < n; i++) parent[i] = i;

        int Find(int x) { while (parent[x] != x) { parent[x] = parent[parent[x]]; x = parent[x]; } return x; }

        for (int k = 0; k < bonds.Length; k++)
            if (!broken[k] && !pulverized[bonds[k].A] && !pulverized[bonds[k].B])
                parent[Find(bonds[k].A)] = Find(bonds[k].B);

        var label = new int[n];
        for (int i = 0; i < n; i++) label[i] = -1;
        count = 0;
        var comp = new int[n];
        for (int i = 0; i < n; i++)
        {
            if (pulverized[i]) { comp[i] = -1; continue; }   // vaporised → in no fragment
            int r = Find(i);
            if (label[r] < 0) label[r] = count++;
            comp[i] = label[r];
        }
        return comp;
    }

    // -------------------------------------------------------------------------
    // Fragment construction
    // -------------------------------------------------------------------------
    private static FragmentSpec[] BuildFragments(
        in FracturableBody body, in FractureInput input,
        int[] comp, int compCount, bool[] broken, bool[] pulverized, float totalArea, Random rng,
        bool includePulverizedDebris = true)
    {
        Cell[] cells = body.Cells;
        Bond[] bonds = body.Bonds;
        FractureProperties mat = body.Material;

        float cos = MathF.Cos(input.BodyRotation), sin = MathF.Sin(input.BodyRotation);
        Vector2 bodyPos = input.BodyPosition;   // local copy: can't capture an 'in' parameter
        Vector2 ToWorld(Vector2 local) => new(
            local.X * cos - local.Y * sin + bodyPos.X,
            local.X * sin + local.Y * cos + bodyPos.Y);

        float refArea = MathF.Max(1f, mat.GrainArea);   // one-cell reference size

        var groups = new List<int>[compCount];
        for (int c = 0; c < compCount; c++) groups[c] = new List<int>();
        for (int i = 0; i < cells.Length; i++) if (comp[i] >= 0) groups[comp[i]].Add(i);

        var result = new List<FragmentSpec>(compCount + 4);

        // --- surviving components → fragment bodies ---
        for (int c = 0; c < compCount; c++)
        {
            List<int> idxs = groups[c];
            if (idxs.Count == 0) continue;

            float area = 0f;
            Vector2 cen = Vector2.Zero;
            foreach (int ci in idxs) { area += cells[ci].Area; cen += cells[ci].Centroid * cells[ci].Area; }
            cen /= area;

            var newCells = new Cell[idxs.Count];
            var remap = new Dictionary<int, int>(idxs.Count);
            for (int k = 0; k < idxs.Count; k++)
            {
                int ci = idxs[k];
                remap[ci] = k;
                Cell src = cells[ci];
                var local = new Vector2[src.Local.Length];
                for (int v = 0; v < local.Length; v++) local[v] = src.Local[v] - cen;
                newCells[k] = new Cell { Local = local, Centroid = src.Centroid - cen, Area = src.Area };
            }

            // Keep only UNBROKEN bonds within the component. Broken-but-still-connected
            // bonds become permanent cracks (progressive damage + visible fissures).
            var newBonds = new List<Bond>();
            for (int bi = 0; bi < bonds.Length; bi++)
            {
                Bond b = bonds[bi];
                if (!broken[bi] && comp[b.A] == c && comp[b.B] == c)
                    newBonds.Add(new Bond { A = remap[b.A], B = remap[b.B], EdgeLength = b.EdgeLength, Strength = b.Strength });
            }

            float mass = input.BodyMass * (area / totalArea);
            float inertia = InertiaAbout(newCells, mass);

            Vector2 worldCentroid = ToWorld(cen);
            var (linear, angular) = FragmentMotion(input, worldCentroid, area, refArea, mat, rng, debris: false);

            result.Add(new FragmentSpec
            {
                Body = new FracturableBody
                {
                    Cells = newCells,
                    Bonds = newBonds.ToArray(),
                    Material = mat,
                    State = new FractureState { AbsorbedEnergy = 0f, RngSeed = (uint)rng.Next() },
                },
                WorldCentroid = worldCentroid,
                Rotation = input.BodyRotation,   // cells are parent-local un-rotated → inherit rotation
                Linear = linear,
                Angular = angular,
                Mass = mass,
                Inertia = inertia,
                Area = area,
                IsDebris = idxs.Count == 1 && area < mat.MinFragmentArea,
            });
        }

        // --- pulverised cells → debris (the asteroid loses this material) ---
        if (includePulverizedDebris)
            for (int i = 0; i < cells.Length; i++)
            {
                if (!pulverized[i]) continue;
                Vector2 worldCentroid = ToWorld(cells[i].Centroid);
                var (linear, angular) = FragmentMotion(input, worldCentroid, cells[i].Area, refArea, mat, rng, debris: true);
                result.Add(new FragmentSpec
                {
                    WorldCentroid = worldCentroid,
                    Rotation = input.BodyRotation,
                    Linear = linear,
                    Angular = angular,
                    Mass = input.BodyMass * (cells[i].Area / totalArea),
                    Inertia = 0f,
                    Area = cells[i].Area,
                    IsDebris = true,
                });
            }

        return result.ToArray();
    }

    /// <summary>Fragment linear + angular velocity: parent drift, ω×r carry-over,
    /// directional push along the shot, radial scatter, and an impact-induced shear
    /// spin (deterministic per side, not random).</summary>
    private static (Vector2 linear, float angular) FragmentMotion(
        in FractureInput input, Vector2 worldCentroid, float area, float refArea,
        FractureProperties mat, Random rng, bool debris)
    {
        Vector2 r = worldCentroid - input.BodyPosition;
        Vector2 rotVel = new(-input.BodyAngular * r.Y, input.BodyAngular * r.X);   // ω × r
        Vector2 spread = worldCentroid - input.ImpactPointWorld;
        float sl = spread.Length();
        spread = sl > 1e-4f ? spread / sl : RandomUnit(rng);

        float boost = MathF.Sqrt(refArea / MathF.Max(area, refArea * 0.1f));        // smaller → faster
        float spd = input.EjectSpeed * boost * (0.6f + 0.8f * (float)rng.NextDouble()) * (debris ? 1.6f : 1f);
        Vector2 linear = input.BodyLinear + rotVel + input.MomentumKick + spread * spd;

        // Shear spin from the hit: off-axis fragments rotate consistently with the shot
        // (same side → same sign) — cross(spreadDir, shotDir). Plus a little variety.
        float shear = spread.X * input.ImpactDir.Y - spread.Y * input.ImpactDir.X;
        float angular = input.BodyAngular + input.ImpactSpin * shear
                      + (float)(rng.NextDouble() - 0.5) * Lerp(0.4f, 1.2f, mat.Brittleness);
        return (linear, -angular);
    }

    private static float InertiaAbout(Cell[] cells, float mass)
    {
        float total = 0f;
        foreach (var c in cells) total += c.Area;
        if (total <= 0f) return 0f;

        float inertia = 0f;
        foreach (var c in cells)
        {
            float m = mass * (c.Area / total);
            inertia += PolygonUtils.ComputeInertia(c.Local, m) + m * c.Centroid.LengthSquared();
        }
        return inertia;
    }

    // -------------------------------------------------------------------------
    private static float Lerp(float a, float b, float t) => a + (b - a) * t;

    private static Vector2 RotateInv(Vector2 v, float rot)
    {
        float c = MathF.Cos(rot), s = MathF.Sin(rot);
        return new Vector2(v.X * c + v.Y * s, -v.X * s + v.Y * c);
    }

    private static Vector2 RandomUnit(Random rng)
    {
        float a = (float)(rng.NextDouble() * Math.PI * 2.0);
        return new Vector2(MathF.Cos(a), MathF.Sin(a));
    }
}
