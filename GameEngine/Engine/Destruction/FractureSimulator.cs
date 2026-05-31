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
    private const float Transmission0     = 0.18f;  // brittleness 0 → short reach
    private const float Transmission1     = 0.96f;  // brittleness 1 → long reach
    private const float BoostCap          = 3f;     // max directional concentration per bond
    private const float DirExponent       = 1.6f;
    private const float SpinCoeff         = 0.12f;  // centrifugal pre-stress scale (calibrate, spec §9)
    private const float MinEffStrengthFrac = 0.02f;

    public static FractureResult Simulate(in FracturableBody body, in FractureInput input, Random rng)
    {
        Cell[] cells = body.Cells;
        Bond[] bonds = body.Bonds;
        FractureProperties mat = body.Material;

        float totalArea = 0f;
        foreach (var c in cells) totalArea += c.Area;

        float struckMass = input.BodyMass * (cells[input.StruckCell].Area / totalArea);
        float threshold  = mat.Toughness * struckMass;
        float combined   = input.EnergyTotal + body.State.AbsorbedEnergy;

        if (combined < threshold)
            return new FractureResult
            {
                Fractured = false,
                AbsorbedEnergy = body.State.AbsorbedEnergy + input.EnergyTotal,
                ImpactPointWorld = input.ImpactPointWorld,
            };

        float eAvail   = combined - threshold;
        float kFrac    = Lerp(mat.KineticFraction, mat.KineticFraction * 0.3f, mat.Brittleness);
        float eSurface = (1f - kFrac) * eAvail;
        float eKinetic = kFrac * eAvail;

        var eff = new float[bonds.Length];
        ComputeEffStrengths(cells, bonds, input.SpinOmega, eff);

        var adj    = BuildAdjacency(cells.Length, bonds);
        var broken = new bool[bonds.Length];
        Propagate(cells, bonds, adj, eff, broken,
                  input.StruckCell, input.EnergyTotal, eSurface,
                  RotateInv(input.ImpactDir, input.BodyRotation),
                  input.Directionality, mat.Brittleness);

        int[] comp = ConnectedComponents(cells.Length, bonds, broken, out int compCount);
        var fragments = BuildFragments(body, input, comp, compCount, totalArea, eKinetic, rng);

        return new FractureResult
        {
            Fractured        = true,
            AbsorbedEnergy   = 0f,
            Fragments        = fragments,
            ImpactPointWorld = input.ImpactPointWorld,
            EnergySurface    = eSurface,
            EnergyKinetic    = eKinetic,
        };
    }

    // -------------------------------------------------------------------------
    // Spin pre-stress: weaken tangentially-oriented bonds, growing outward.
    // -------------------------------------------------------------------------
    private static void ComputeEffStrengths(Cell[] cells, Bond[] bonds, float omega, float[] eff)
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
            float   r = m.Length();
            if (r < 1e-3f) { eff[k] = b.Strength; continue; }

            Vector2 rad = m / r;
            Vector2 tan = new(-rad.Y, rad.X);
            Vector2 dir = cells[b.B].Centroid - cells[b.A].Centroid;
            float   dl  = dir.Length();
            if (dl > 1e-6f) dir /= dl;

            float tangentiality = MathF.Abs(Vector2.Dot(dir, tan));
            float profile       = 0.3f + 0.7f * (r / rmax);
            float preStress     = SpinCoeff * avg * w2 * profile * tangentiality;
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
        Cell[] cells, Bond[] bonds, List<int>[] adj, float[] eff, bool[] broken,
        int struck, float startEnergy, float budget,
        Vector2 impactDirLocal, float directionality, float brittleness)
    {
        int n = cells.Length;
        var energy    = new float[n];
        var processed = new float[n];
        for (int i = 0; i < n; i++) processed[i] = -1f;
        energy[struck] = startEnergy;

        float transmission = Lerp(Transmission0, Transmission1, brittleness);

        var frontier = new List<int> { struck };
        var outBonds = new List<(int bond, int j, float w)>();

        while (frontier.Count > 0 && budget > 0f)
        {
            int mi = 0;
            for (int k = 1; k < frontier.Count; k++)
                if (energy[frontier[k]] > energy[frontier[mi]]) mi = k;
            int   i = frontier[mi];
            float e = energy[i];
            frontier.RemoveAt(mi);
            if (e <= processed[i]) continue;   // stale / already processed at ≥ this energy
            processed[i] = e;

            outBonds.Clear();
            float sumW = 0f;
            foreach (int bk in adj[i])
            {
                if (broken[bk]) continue;
                int j = bonds[bk].A == i ? bonds[bk].B : bonds[bk].A;
                Vector2 d = cells[j].Centroid - cells[i].Centroid;
                float dl = d.Length();
                if (dl > 1e-6f) d /= dl;
                float align = Vector2.Dot(d, impactDirLocal);
                float w = Lerp(1f, MathF.Pow(MathF.Max(0f, align), DirExponent), directionality);
                outBonds.Add((bk, j, w));
                sumW += w;
            }
            if (outBonds.Count == 0) continue;

            outBonds.Sort((x, y) => y.w.CompareTo(x.w));      // spend on most-aligned first
            float norm = outBonds.Count / MathF.Max(sumW, 1e-6f);   // mean weight 1: steer, don't attenuate

            foreach (var (bk, j, w) in outBonds)
            {
                if (budget <= 0f) break;
                float deliver = e * MathF.Min(w * norm, BoostCap);
                if (deliver > eff[bk])
                {
                    broken[bk] = true;
                    budget -= eff[bk];
                    float tr = (deliver - eff[bk]) * transmission;
                    if (tr > e) tr = e;                          // child never exceeds parent
                    if (tr > energy[j]) { energy[j] = tr; frontier.Add(j); }
                }
            }
        }
    }

    private static int[] ConnectedComponents(int n, Bond[] bonds, bool[] broken, out int count)
    {
        var parent = new int[n];
        for (int i = 0; i < n; i++) parent[i] = i;

        int Find(int x) { while (parent[x] != x) { parent[x] = parent[parent[x]]; x = parent[x]; } return x; }

        for (int k = 0; k < bonds.Length; k++)
            if (!broken[k]) parent[Find(bonds[k].A)] = Find(bonds[k].B);

        var label = new int[n];
        for (int i = 0; i < n; i++) label[i] = -1;
        count = 0;
        var comp = new int[n];
        for (int i = 0; i < n; i++)
        {
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
        int[] comp, int compCount, float totalArea, float eKinetic, Random rng)
    {
        Cell[] cells = body.Cells;
        Bond[] bonds = body.Bonds;
        FractureProperties mat = body.Material;

        var groups = new List<int>[compCount];
        for (int c = 0; c < compCount; c++) groups[c] = new List<int>();
        for (int i = 0; i < cells.Length; i++) groups[comp[i]].Add(i);

        float cos = MathF.Cos(input.BodyRotation), sin = MathF.Sin(input.BodyRotation);
        Vector2 bodyPos = input.BodyPosition;   // local copy: can't capture an 'in' parameter
        Vector2 ToWorld(Vector2 local) => new(
            local.X * cos - local.Y * sin + bodyPos.X,
            local.X * sin + local.Y * cos + bodyPos.Y);

        float kePer = compCount > 0 ? eKinetic / compCount : 0f;   // equal KE share → lighter fly faster

        var result = new FragmentSpec[compCount];
        for (int c = 0; c < compCount; c++)
        {
            List<int> idxs = groups[c];

            float   area = 0f;
            Vector2 cen  = Vector2.Zero;
            foreach (int ci in idxs) { area += cells[ci].Area; cen += cells[ci].Centroid * cells[ci].Area; }
            cen /= area;

            // Re-centre this component's cells to its own centroid.
            var newCells = new Cell[idxs.Count];
            var remap    = new Dictionary<int, int>(idxs.Count);
            for (int k = 0; k < idxs.Count; k++)
            {
                int ci = idxs[k];
                remap[ci] = k;
                Cell src = cells[ci];
                var local = new Vector2[src.Local.Length];
                for (int v = 0; v < local.Length; v++) local[v] = src.Local[v] - cen;
                newCells[k] = new Cell { Local = local, Centroid = src.Centroid - cen, Area = src.Area };
            }

            var newBonds = new List<Bond>();
            foreach (Bond b in bonds)
                if (comp[b.A] == c && comp[b.B] == c)
                    newBonds.Add(new Bond { A = remap[b.A], B = remap[b.B], EdgeLength = b.EdgeLength, Strength = b.Strength });

            float mass    = input.BodyMass * (area / totalArea);
            float inertia = InertiaAbout(newCells, mass);

            Vector2 worldCentroid = ToWorld(cen);
            Vector2 r       = worldCentroid - input.BodyPosition;
            Vector2 rotVel  = new(-input.BodyAngular * r.Y, input.BodyAngular * r.X);   // ω × r carry-over
            Vector2 spread  = worldCentroid - input.ImpactPointWorld;
            float   sl      = spread.Length();
            spread = sl > 1e-4f ? spread / sl : RandomUnit(rng);
            float   spreadSpeed = mass > 1e-4f ? MathF.Sqrt(2f * kePer / mass) : 0f;

            Vector2 linear  = input.BodyLinear + input.MomentumKick + rotVel + spread * spreadSpeed;
            float   angular = input.BodyAngular + (float)(rng.NextDouble() - 0.5) * 2f * Lerp(0.5f, 2.5f, mat.Brittleness);

            bool isDebris = idxs.Count == 1 && area < mat.MinFragmentArea;

            result[c] = new FragmentSpec
            {
                Body = new FracturableBody
                {
                    Cells    = newCells,
                    Bonds    = newBonds.ToArray(),
                    Material = mat,
                    State    = new FractureState { AbsorbedEnergy = 0f, RngSeed = (uint)rng.Next() },
                },
                WorldCentroid = worldCentroid,
                Rotation = input.BodyRotation,   // cells are parent-local un-rotated → inherit rotation
                Linear  = linear,
                Angular = angular,
                Mass    = mass,
                Inertia = inertia,
                Area    = area,
                IsDebris = isDebris,
            };
        }
        return result;
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
