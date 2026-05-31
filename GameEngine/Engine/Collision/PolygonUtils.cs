using System.Numerics;

namespace AsteroidsEngine.Engine.Collision;

/// <summary>
/// Result of a Split() call. All polygon arrays are convex by the SH invariant.
/// Callers assign PolygonShape directly — no further decomposition needed.
/// </summary>
public readonly struct SplitResult
{
    /// <summary>
    /// Far polygon from the primary perpendicular cut. The largest surviving chunk.
    /// Null if the entire polygon fell inside the fracture zone.
    /// </summary>
    public readonly Vector2[]? PrimaryFarPiece;

    /// <summary>
    /// Far polygons from the K-1 secondary radial cuts (the "petal" pieces).
    /// Together with PrimaryFarPiece these form the surviving compound asteroid.
    /// </summary>
    public readonly Vector2[][] SecondaryFarPieces;

    /// <summary>Impact-zone pieces with area ≥ minAreaThreshold AND centroid > blastRadius.</summary>
    public readonly Vector2[][] SurvivingFragments;

    /// <summary>Impact-zone pieces with area &lt; minAreaThreshold OR centroid ≤ blastRadius → fading particles.</summary>
    public readonly Vector2[][] DebrisFragments;

    public SplitResult(Vector2[]?  primaryFarPiece,
                       Vector2[][] secondaryFarPieces,
                       Vector2[][] survivingFragments,
                       Vector2[][] debrisFragments)
    {
        PrimaryFarPiece    = primaryFarPiece;
        SecondaryFarPieces = secondaryFarPieces;
        SurvivingFragments = survivingFragments;
        DebrisFragments    = debrisFragments;
    }
}

/// <summary>
/// Pure-geometry utilities for convex polygon generation, clipping, splitting,
/// and physical property computation. No ECS, no rendering dependencies.
///
/// Coordinate convention: matches the engine's Y-down screen space.
/// Winding order produced by GenerateConvex: clockwise (matches PolygonShape).
///
/// Critical invariant maintained by every caller:
///   PolygonShape vertices must be centroid-relative (local space).
///   Transform.Position IS the world centroid.
/// Use RecenterVertices() after any split to re-establish this invariant.
/// </summary>
public static class PolygonUtils
{
    // -------------------------------------------------------------------------
    // Generation
    // -------------------------------------------------------------------------

    /// <summary>
    /// Generates a random convex polygon centred at the origin with clockwise winding.
    ///
    /// Uses Valtr's algorithm, which produces a genuinely random convex polygon with
    /// exactly <paramref name="sides"/> vertices. (Naively placing vertices at sorted
    /// angles with random radii does NOT preserve convexity — a short-radius vertex
    /// between two long ones forms a reflex corner.) The result is scaled so the mean
    /// vertex distance from the centroid equals <paramref name="radius"/>.
    /// </summary>
    /// <param name="radiusVariation">
    /// Retained for API compatibility; Valtr's algorithm produces its own natural
    /// irregularity so this value is not used directly.
    /// </param>
    public static (Vector2[] vertices, float[] faultAngles) GenerateConvex(
        int sides,
        float radius,
        Random rng,
        int faultCount = 3,
        float radiusVariation = 0.22f)
    {
        if (sides < 3) throw new ArgumentOutOfRangeException(nameof(sides), "Need at least 3 sides.");

        var verts = RandomConvexPolygon(sides, rng);

        // Re-centre on the area centroid, then scale to the requested mean radius.
        var (_, centred) = RecenterVertices(verts);
        float meanR = 0f;
        foreach (var v in centred) meanR += v.Length();
        meanR /= centred.Length;
        float scale = meanR > 1e-6f ? radius / meanR : 1f;
        for (int i = 0; i < centred.Length; i++) centred[i] *= scale;

        // Clockwise winding (engine convention, Y-down). Positive shoelace area = CCW
        // in Y-up math; reverse to make it CW.
        if (ComputeArea(centred) > 0f) Array.Reverse(centred);

        var faults = new float[faultCount];
        for (int i = 0; i < faultCount; i++)
            faults[i] = (float)(rng.NextDouble() * 2 * MathF.PI);

        return (centred, faults);
    }

    /// <summary>
    /// Valtr's algorithm for a uniformly random convex polygon with exactly n vertices.
    /// Builds n edge vectors whose x- and y-components each sum to zero (so the chain
    /// closes), sorts them by angle, and connects them head-to-tail. Sorting by angle
    /// guarantees the result is convex. Output is roughly in a unit-scale box around origin.
    /// </summary>
    private static Vector2[] RandomConvexPolygon(int n, Random rng)
    {
        var xs = new float[n];
        var ys = new float[n];
        for (int i = 0; i < n; i++) { xs[i] = (float)rng.NextDouble(); ys[i] = (float)rng.NextDouble(); }
        Array.Sort(xs);
        Array.Sort(ys);

        float minX = xs[0], maxX = xs[n - 1];
        float minY = ys[0], maxY = ys[n - 1];

        // Split the interior points of each coordinate into two monotone chains,
        // producing signed component vectors that sum to zero.
        var xVec = new float[n];
        var yVec = new float[n];

        float lastTop = minX, lastBot = minX;
        for (int i = 1; i < n - 1; i++)
        {
            float x = xs[i];
            if (rng.Next(2) == 0) { xVec[i - 1] = x - lastTop; lastTop = x; }
            else                  { xVec[i - 1] = lastBot - x; lastBot = x; }
        }
        xVec[n - 2] = maxX - lastTop;
        xVec[n - 1] = lastBot - maxX;

        float lastLeft = minY, lastRight = minY;
        for (int i = 1; i < n - 1; i++)
        {
            float y = ys[i];
            if (rng.Next(2) == 0) { yVec[i - 1] = y - lastLeft;  lastLeft = y; }
            else                  { yVec[i - 1] = lastRight - y; lastRight = y; }
        }
        yVec[n - 2] = maxY - lastLeft;
        yVec[n - 1] = lastRight - maxY;

        // Randomly pair x- and y-components (shuffle y), forming edge vectors.
        for (int i = n - 1; i > 0; i--)
        {
            int j = rng.Next(i + 1);
            (yVec[i], yVec[j]) = (yVec[j], yVec[i]);
        }

        var vecs = new Vector2[n];
        for (int i = 0; i < n; i++) vecs[i] = new Vector2(xVec[i], yVec[i]);

        // Sort edge vectors by angle → convex chain.
        Array.Sort(vecs, (a, b) => MathF.Atan2(a.Y, a.X).CompareTo(MathF.Atan2(b.Y, b.X)));

        // Connect head-to-tail.
        var pts = new Vector2[n];
        Vector2 cur = Vector2.Zero;
        for (int i = 0; i < n; i++) { pts[i] = cur; cur += vecs[i]; }
        return pts;
    }

    // -------------------------------------------------------------------------
    // Sutherland-Hodgman half-plane clip
    // -------------------------------------------------------------------------

    /// <summary>
    /// Clips a convex polygon against a half-plane, keeping the side where
    ///   dot(point − planePoint, planeNormal) ≥ 0
    /// Returns an empty list if the entire polygon is outside.
    /// </summary>
    public static List<Vector2> ClipConvexByHalfPlane(
        IReadOnlyList<Vector2> polygon,
        Vector2 planePoint,
        Vector2 planeNormal)
    {
        var output = new List<Vector2>(polygon.Count + 1);
        int n = polygon.Count;
        if (n == 0) return output;

        for (int i = 0; i < n; i++)
        {
            Vector2 curr = polygon[i];
            Vector2 next = polygon[(i + 1) % n];

            float dCurr = Vector2.Dot(curr - planePoint, planeNormal);
            float dNext = Vector2.Dot(next - planePoint, planeNormal);

            bool currIn = dCurr >= 0f;
            bool nextIn = dNext >= 0f;

            if (currIn) output.Add(curr);

            if (currIn != nextIn)
            {
                float t = dCurr / (dCurr - dNext);
                output.Add(curr + t * (next - curr));
            }
        }

        return output;
    }

    // -------------------------------------------------------------------------
    // Two-phase polygon splitting
    // -------------------------------------------------------------------------

    /// <summary>
    /// Splits a convex polygon into fragments using a three-phase algorithm.
    ///
    /// Phase 1 — Primary cut: one plane perpendicular to the centroid→impact direction,
    /// at <paramref name="fractureZoneDepth"/> from the impact point. Produces
    /// PrimaryFarPiece (surviving chunk) + fracture zone (near side).
    ///
    /// Phase 2 — Radial secondary cuts: <paramref name="secondaryCuts"/> planes are
    /// applied sequentially to the fracture zone. Each is perpendicular to a ray
    /// from the impact point, fanned across a cone of half-angle
    /// <paramref name="coneHalfAngle"/> facing the centroid. Each cut's far side
    /// becomes a SecondaryFarPiece (also attached to the surviving compound).
    /// Together the far pieces form a concave "bite" at the impact site.
    ///
    /// Phase 3 — Impact zone fragmentation: the remaining inner polygon is cut by
    /// <paramref name="innerCuts"/> planes. Fragments within
    /// <paramref name="blastRadius"/> of the impact point go to DebrisFragments.
    ///
    /// All output polygons are convex (SH invariant). Spawn blast particles at
    /// the impact point directly — no geometry extraction needed.
    /// </summary>
    public static SplitResult Split(
        IReadOnlyList<Vector2> polygon,
        Vector2 impactPoint,
        Vector2 impactDir,
        float[] faultAngles,
        int  secondaryCuts,
        int  innerCuts,
        float fractureZoneDepth,
        float fractureRadius,
        float coneHalfAngle,
        float blastRadius,
        float spreadAngle,
        float spinFaultAngle,
        Random rng,
        float minAreaThreshold = 180f)
    {
        secondaryCuts = Math.Clamp(secondaryCuts, 0, 8);
        innerCuts     = Math.Clamp(innerCuts,     1, 10);

        // ── Phase 1: primary perpendicular cut ───────────────────────────────
        Vector2 centroid = ComputeCentroid(polygon);
        Vector2 toImpact = impactPoint - centroid;
        Vector2 cutDir   = toImpact.LengthSquared() > 1e-4f
                           ? Vector2.Normalize(toImpact)
                           : impactDir;

        Vector2 primaryCutPoint = impactPoint - cutDir * fractureZoneDepth;
        var primaryFarPiece = ClipConvexByHalfPlane(polygon, primaryCutPoint, -cutDir);
        List<Vector2> zone  = ClipConvexByHalfPlane(polygon, primaryCutPoint,  cutDir);

        // ── Phase 2: secondary radial cuts (the "bite") ──────────────────────
        // innerDir: from impact toward centroid — the centre of the cut fan.
        Vector2 innerDir  = -cutDir;
        float   impactAngle = MathF.Atan2(innerDir.Y, innerDir.X);
        float   step        = secondaryCuts > 1
                              ? 2f * coneHalfAngle / (secondaryCuts - 1)
                              : 0f;

        var secondaryFarPieces = new List<Vector2[]>(secondaryCuts);

        for (int k = 0; k < secondaryCuts && zone.Count >= 3; k++)
        {
            float   angle = impactAngle + (k - (secondaryCuts - 1) * 0.5f) * step;
            Vector2 ray   = new Vector2(MathF.Cos(angle), MathF.Sin(angle));
            Vector2 pt    = impactPoint + ray * fractureRadius;

            // far side of cut (away from impact along ray) → surviving compound
            var petal = ClipConvexByHalfPlane(zone, pt,  ray);
            zone      = ClipConvexByHalfPlane(zone, pt, -ray);

            if (petal.Count >= 3 && MathF.Abs(ComputeArea(petal)) >= minAreaThreshold * 0.5f)
                secondaryFarPieces.Add(petal.ToArray());
        }

        // ── Phase 3: impact zone fragmentation ───────────────────────────────
        var surviving = new List<Vector2[]>();
        var debris    = new List<Vector2[]>();

        if (zone.Count >= 3)
            ProcessImpactZone(zone, impactPoint, blastRadius, faultAngles, innerCuts,
                              spreadAngle, spinFaultAngle, minAreaThreshold, rng,
                              surviving, debris);

        Vector2[]? primaryFar = primaryFarPiece.Count >= 3 ? primaryFarPiece.ToArray() : null;
        return new SplitResult(primaryFar, secondaryFarPieces.ToArray(),
                               surviving.ToArray(), debris.ToArray());
    }

    /// <summary>
    /// Applies inner cuts to the impact zone and classifies each resulting fragment:
    ///   • centroid within blastRadius of impactPoint → DebrisFragments (blast particles)
    ///   • area &lt; minAreaThreshold → DebrisFragments (dust)
    ///   • otherwise → SurvivingFragments
    /// Cut planes all pass through impactPoint at angles chosen by SelectCutAngles.
    /// </summary>
    private static void ProcessImpactZone(
        IReadOnlyList<Vector2> zone,
        Vector2 impactPoint,
        float blastRadius,
        float[] faultAngles,
        int innerCuts,
        float spreadAngle,
        float spinFaultAngle,
        float minAreaThreshold,
        Random rng,
        List<Vector2[]> surviving,
        List<Vector2[]> debris)
    {
        if (zone.Count < 3) return;

        float[] cutAngles = SelectCutAngles(zone, impactPoint, faultAngles,
                                            innerCuts, spreadAngle, spinFaultAngle, rng);

        var fragments = new List<List<Vector2>> { new List<Vector2>(zone) };

        foreach (float angle in cutAngles)
        {
            var dir    = new Vector2(MathF.Cos(angle), MathF.Sin(angle));
            var normal = new Vector2(-dir.Y, dir.X);
            var next   = new List<List<Vector2>>(fragments.Count * 2);

            foreach (var frag in fragments)
            {
                var a = ClipConvexByHalfPlane(frag, impactPoint,  normal);
                var b = ClipConvexByHalfPlane(frag, impactPoint, -normal);
                if (a.Count >= 3) next.Add(a);
                if (b.Count >= 3) next.Add(b);
            }
            fragments = next;
        }

        float blastR2 = blastRadius * blastRadius;

        foreach (var frag in fragments)
        {
            if (frag.Count < 3) continue;

            // Blast zone filter: centroid within blastRadius → particle
            Vector2 fragCentroid = ComputeCentroid(frag);
            if (blastR2 > 0f && (fragCentroid - impactPoint).LengthSquared() <= blastR2)
            {
                debris.Add(frag.ToArray());
                continue;
            }

            float area = MathF.Abs(ComputeArea(frag));
            if (area >= minAreaThreshold)
                surviving.Add(frag.ToArray());
            else
                debris.Add(frag.ToArray());
        }
    }

    // -------------------------------------------------------------------------
    // Geometric properties
    // -------------------------------------------------------------------------

    /// <summary>
    /// Signed area via the shoelace formula.
    /// Positive → CCW in Y-up (math); the same vertex order is CW in Y-down (screen).
    /// </summary>
    public static float ComputeArea(IReadOnlyList<Vector2> verts)
    {
        float area = 0f;
        int n = verts.Count;
        for (int i = 0; i < n; i++)
        {
            var a = verts[i];
            var b = verts[(i + 1) % n];
            area += a.X * b.Y - b.X * a.Y;
        }
        return area * 0.5f;
    }

    /// <summary>Area centroid via the shoelace triangulation formula.</summary>
    public static Vector2 ComputeCentroid(IReadOnlyList<Vector2> verts)
    {
        var c = Vector2.Zero;
        float area = 0f;
        int n = verts.Count;

        for (int i = 0; i < n; i++)
        {
            var a = verts[i];
            var b = verts[(i + 1) % n];
            float cross = a.X * b.Y - b.X * a.Y;
            area += cross;
            c += (a + b) * cross;
        }

        area *= 0.5f;
        if (MathF.Abs(area) < 1e-6f)
        {
            var sum = Vector2.Zero;
            foreach (var v in verts) sum += v;
            return sum / n;
        }

        return c / (6f * area);
    }

    /// <summary>Moment of inertia about the centroid for a solid uniform polygon.</summary>
    public static float ComputeInertia(IReadOnlyList<Vector2> centroidRelativeVerts, float mass)
    {
        float area = MathF.Abs(ComputeArea(centroidRelativeVerts));
        if (area < 1e-6f) return 0f;

        float density = mass / area;
        float inertia = 0f;
        int n = centroidRelativeVerts.Count;

        for (int i = 0; i < n; i++)
        {
            var a = centroidRelativeVerts[i];
            var b = centroidRelativeVerts[(i + 1) % n];
            float cross = MathF.Abs(a.X * b.Y - b.X * a.Y);
            inertia += cross * (Vector2.Dot(a, a) + Vector2.Dot(a, b) + Vector2.Dot(b, b));
        }

        return density * inertia / 12f;
    }

    // -------------------------------------------------------------------------
    // Centroid invariant helper
    // -------------------------------------------------------------------------

    /// <summary>
    /// Re-centres world-space vertices around their area centroid.
    /// Returns (centroid, centroid-relative local vertices).
    /// Call after every Split() to maintain Transform.Position == world centroid.
    /// </summary>
    public static (Vector2 centroid, Vector2[] localVertices) RecenterVertices(
        IReadOnlyList<Vector2> worldVertices)
    {
        Vector2 centroid = ComputeCentroid(worldVertices);
        var local = new Vector2[worldVertices.Count];
        for (int i = 0; i < worldVertices.Count; i++)
            local[i] = worldVertices[i] - centroid;
        return (centroid, local);
    }

    // -------------------------------------------------------------------------
    // Surface projection
    // -------------------------------------------------------------------------

    /// <summary>
    /// Returns the nearest point on the polygon boundary to <paramref name="point"/>.
    /// The result is always on an edge of the polygon, never inside it.
    /// Use this to correct a contact point that may have tunnelled into the interior.
    /// </summary>
    public static Vector2 NearestPointOnBoundary(IReadOnlyList<Vector2> polygon, Vector2 point)
    {
        float bestDist = float.MaxValue;
        Vector2 best = polygon[0];
        int n = polygon.Count;
        for (int i = 0; i < n; i++)
        {
            Vector2 a  = polygon[i];
            Vector2 b  = polygon[(i + 1) % n];
            Vector2 ab = b - a;
            float lenSq = ab.LengthSquared();
            float t     = lenSq > 1e-10f
                          ? Math.Clamp(Vector2.Dot(point - a, ab) / lenSq, 0f, 1f)
                          : 0f;
            Vector2 proj = a + t * ab;
            float d = (proj - point).LengthSquared();
            if (d < bestDist) { bestDist = d; best = proj; }
        }
        return best;
    }

    // -------------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------------

    /// <summary>
    /// Distributes count cut angles evenly within spreadAngle, centred on the
    /// direction perpendicular to (impactPoint − shardCentroid), then snaps each
    /// slot to the nearest fault angle or spin-induced fault within half a slot width.
    /// </summary>
    private static float[] SelectCutAngles(
        IReadOnlyList<Vector2> shard,
        Vector2 impactPoint,
        float[] faultAngles,
        int count,
        float spreadAngle,
        float spinFaultAngle,
        Random rng)
    {
        Vector2 centroid  = ComputeCentroid(shard);
        Vector2 toImpact  = impactPoint - centroid;
        float baseAngle = toImpact.LengthSquared() > 1e-6f
            ? MathF.Atan2(toImpact.Y, toImpact.X) + MathF.PI / 2f
            : (float)(rng.NextDouble() * 2 * MathF.PI);

        float slotWidth = count > 1 ? spreadAngle / count : spreadAngle;
        float halfSlot  = slotWidth * 0.5f;
        var result = new float[count];

        for (int i = 0; i < count; i++)
        {
            float slotCenter = baseAngle + (i - (count - 1) * 0.5f) * slotWidth;

            float best     = slotCenter;
            float bestDist = halfSlot + 1f;

            // Snap to nearest fault angle within the slot (treat as undirected line).
            foreach (float fa in faultAngles)
            {
                float d = MathF.Min(AngleDiff(fa, slotCenter),
                                    AngleDiff(fa + MathF.PI, slotCenter));
                if (d < bestDist && d < halfSlot)
                {
                    bestDist = d;
                    best = AngleDiff(fa, slotCenter) <= AngleDiff(fa + MathF.PI, slotCenter)
                           ? fa : fa + MathF.PI;
                }
            }

            // Also snap to spin-induced fault if closer.
            if (!float.IsNaN(spinFaultAngle))
            {
                float d = MathF.Min(AngleDiff(spinFaultAngle, slotCenter),
                                    AngleDiff(spinFaultAngle + MathF.PI, slotCenter));
                if (d < bestDist && d < halfSlot)
                {
                    bestDist = d;
                    best = AngleDiff(spinFaultAngle, slotCenter)
                           <= AngleDiff(spinFaultAngle + MathF.PI, slotCenter)
                           ? spinFaultAngle : spinFaultAngle + MathF.PI;
                }
            }

            result[i] = best + (float)(rng.NextDouble() - 0.5) * 0.25f;
        }

        return result;
    }

    private static float AngleDiff(float a, float b)
    {
        float d = MathF.Abs(a - b) % (2 * MathF.PI);
        return d > MathF.PI ? 2 * MathF.PI - d : d;
    }
}
