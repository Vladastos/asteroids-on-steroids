using System.Numerics;
using AsteroidsEngine.Engine.Collision;
using AsteroidsEngine.Engine.Components;
using AsteroidsEngine.Engine.Core;

namespace AsteroidsEngine.Engine.Systems;

/// <summary>A raycast hit against a specific entity's collider.</summary>
public readonly struct RayHit
{
    public readonly Entity  Entity;
    public readonly Vector2 Point;
    public readonly Vector2 Normal;
    public readonly float   Distance;
    public readonly int     PartIndex;   // CompoundShape part struck; -1 for simple shapes

    public RayHit(Entity entity, Vector2 point, Vector2 normal, float distance, int partIndex)
    {
        Entity    = entity;
        Point     = point;
        Normal    = normal;
        Distance  = distance;
        PartIndex = partIndex;
    }
}

/// <summary>
/// Spatial queries against the collision world. Raycasting underpins raycast
/// bullets (no tunnelling, exact impact point + normal + struck part).
/// </summary>
public static class PhysicsQueries
{
    /// <summary>
    /// Casts a ray from <paramref name="from"/> to <paramref name="to"/> and returns
    /// the nearest collider hit whose Collider.Layer intersects <paramref name="layerMask"/>.
    ///
    /// Broad phase is currently a brute-force scan of colliders — fine at these
    /// entity counts; a spatial-index-accelerated path can replace it later without
    /// changing this signature.
    /// </summary>
    public static bool Raycast(World world, Vector2 from, Vector2 to, int layerMask, out RayHit hit)
    {
        hit = default;

        Vector2 delta   = to - from;
        float   maxDist = delta.Length();
        if (maxDist < 1e-6f) return false;
        Vector2 dir = delta / maxDist;

        bool    found   = false;
        float   best    = maxDist;
        RayHit  bestHit = default;

        world.ForEach<Transform, Collider>((Entity e, ref Transform t, ref Collider c) =>
        {
            if ((c.Layer & layerMask) == 0) return;
            if (world.HasComponent<DisabledTag>(e)) return;

            if (c.Shape.Raycast(from, dir, best, t.Position, t.Rotation, out var r))
            {
                best    = r.Distance;   // tighten so we keep only the nearest
                bestHit = new RayHit(e, r.Point, r.Normal, r.Distance, r.PartIndex);
                found   = true;
            }
        });

        hit = bestHit;
        return found;
    }

    /// <summary>True if a circle (centre + radius) overlaps any existing collider on
    /// <paramref name="layerMask"/>. Conservative (tests the circle against each collider's AABB), so
    /// it may reject a near-miss — ideal for spawn placement, where the cost of overlap (stuck/jitter)
    /// far outweighs an occasional retry. Disabled colliders are ignored.</summary>
    public static bool OverlapsCircle(World world, Vector2 centre, float radius, int layerMask)
    {
        bool hit = false;
        float r2 = radius * radius;
        world.ForEach<Transform, Collider>((Entity e, ref Transform t, ref Collider c) =>
        {
            if (hit) return;
            if ((c.Layer & layerMask) == 0) return;
            if (world.HasComponent<DisabledTag>(e)) return;

            var (min, max) = c.Shape.GetAABB(t.Position, t.Rotation);
            // Closest point on the AABB to the circle centre.
            float cx = Math.Clamp(centre.X, min.X, max.X);
            float cy = Math.Clamp(centre.Y, min.Y, max.Y);
            float dx = centre.X - cx, dy = centre.Y - cy;
            if (dx * dx + dy * dy < r2) hit = true;
        });
        return hit;
    }
}
