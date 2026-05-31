using System.Numerics;

namespace AsteroidsEngine.Engine.Collision;

/// <summary>
/// A collision shape composed of multiple convex child shapes.
/// Enables concave-capable collision without modifying the SAT pipeline:
/// each part dispatches through the existing double-dispatch table.
///
/// All parts share the entity's Transform (pos/rot). Parts are centroid-relative,
/// matching the convention used by PolygonShape.
///
/// Thread safety: LastHitPartIndex is written during Intersects() and read
/// immediately after in the single-threaded event handler. Do not cache across frames.
/// </summary>
public sealed class CompoundShape : CollisionShape
{
    private readonly CollisionShape[] _parts;

    public int PartCount => _parts.Length;

    /// <summary>
    /// Index into _parts of the child that produced the deepest contact in the
    /// most recent Intersects() call. -1 if no contact was found.
    /// </summary>
    public int LastHitPartIndex { get; private set; } = -1;

    public CompoundShape(CollisionShape[] parts)
    {
        if (parts.Length == 0) throw new ArgumentException("CompoundShape needs at least one part.");
        _parts = parts;
    }

    public CollisionShape GetPart(int index) => _parts[index];

    /// <summary>Returns a new CompoundShape with the part at index removed.</summary>
    public CompoundShape WithoutPart(int index)
    {
        var remaining = new CollisionShape[_parts.Length - 1];
        int j = 0;
        for (int i = 0; i < _parts.Length; i++)
            if (i != index) remaining[j++] = _parts[i];
        return new CompoundShape(remaining);
    }

    // -------------------------------------------------------------------------
    // CompoundShape as entity A (primary shape)
    // -------------------------------------------------------------------------

    public override ContactInfo? Intersects(Vector2 posA, float rotA,
                                            CollisionShape other,
                                            Vector2 posB, float rotB)
    {
        ContactInfo? deepest = null;
        LastHitPartIndex = -1;
        for (int i = 0; i < _parts.Length; i++)
        {
            var c = _parts[i].Intersects(posA, rotA, other, posB, rotB);
            if (c != null && (deepest == null || c.Value.Depth > deepest.Value.Depth))
            {
                deepest = c;
                LastHitPartIndex = i;
            }
        }
        return deepest;
    }

    public override bool Raycast(Vector2 origin, Vector2 dir, float maxDist,
                                 Vector2 pos, float rot, out RayCastResult hit)
    {
        hit = default;
        bool  any  = false;
        float best = maxDist;
        for (int i = 0; i < _parts.Length; i++)
        {
            if (_parts[i].Raycast(origin, dir, best, pos, rot, out var h))
            {
                best = h.Distance;
                hit  = new RayCastResult(h.Distance, h.Point, h.Normal, partIndex: i);
                any  = true;
            }
        }
        return any;
    }

    public override (Vector2 min, Vector2 max) GetAABB(Vector2 pos, float rot)
    {
        var min = new Vector2(float.MaxValue);
        var max = new Vector2(float.MinValue);
        foreach (var part in _parts)
        {
            var (pmin, pmax) = part.GetAABB(pos, rot);
            min = Vector2.Min(min, pmin);
            max = Vector2.Max(max, pmax);
        }
        return (min, max);
    }

    // -------------------------------------------------------------------------
    // CompoundShape as entity B (double-dispatch target)
    // Each method fans the test across parts and returns the deepest contact.
    // -------------------------------------------------------------------------

    internal override ContactInfo? IntersectsCircle(Vector2 posA, float rotA,
                                                    CircleShape circle, Vector2 posB)
    {
        ContactInfo? deepest = null;
        LastHitPartIndex = -1;
        for (int i = 0; i < _parts.Length; i++)
        {
            var c = _parts[i].IntersectsCircle(posA, rotA, circle, posB);
            if (c != null && (deepest == null || c.Value.Depth > deepest.Value.Depth))
            {
                deepest = c;
                LastHitPartIndex = i;
            }
        }
        return deepest;
    }

    internal override ContactInfo? IntersectsPolygon(Vector2 posA, float rotA,
                                                     PolygonShape polygon,
                                                     Vector2 posB, float rotB)
    {
        ContactInfo? deepest = null;
        LastHitPartIndex = -1;
        for (int i = 0; i < _parts.Length; i++)
        {
            var c = _parts[i].IntersectsPolygon(posA, rotA, polygon, posB, rotB);
            if (c != null && (deepest == null || c.Value.Depth > deepest.Value.Depth))
            {
                deepest = c;
                LastHitPartIndex = i;
            }
        }
        return deepest;
    }

    internal override ContactInfo? IntersectsAABB(Vector2 posA, float rotA,
                                                  AABBShape aabb, Vector2 posB)
    {
        ContactInfo? deepest = null;
        LastHitPartIndex = -1;
        for (int i = 0; i < _parts.Length; i++)
        {
            var c = _parts[i].IntersectsAABB(posA, rotA, aabb, posB);
            if (c != null && (deepest == null || c.Value.Depth > deepest.Value.Depth))
            {
                deepest = c;
                LastHitPartIndex = i;
            }
        }
        return deepest;
    }
}
