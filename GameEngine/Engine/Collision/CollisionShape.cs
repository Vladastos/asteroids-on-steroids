using System.Numerics;

namespace AsteroidsEngine.Engine.Collision;

/// <summary>
/// Abstract collision shape. Each concrete subclass implements
/// Intersects via double dispatch: A.Intersects(B) calls
/// B.IntersectsCircle / B.IntersectsPolygon / etc. so each pair
/// gets the most specific algorithm without a type-switch in the caller.
/// </summary>
public abstract class CollisionShape
{
    /// <summary>
    /// Tests whether this shape (at worldPosition/rotation) overlaps other.
    /// Returns null if no collision, or a ContactInfo where Normal points
    /// from other into this shape.
    /// </summary>
    public abstract ContactInfo? Intersects(Vector2 posA, float rotA,
                                            CollisionShape other,
                                            Vector2 posB, float rotB);

    /// <summary>Axis-aligned bounding box for broad-phase culling.</summary>
    public abstract (Vector2 min, Vector2 max) GetAABB(Vector2 pos, float rot);

    // ---- Double-dispatch entry points (called by concrete subclasses) ----
    internal abstract ContactInfo? IntersectsCircle (Vector2 posA, float rotA, CircleShape  circle,  Vector2 posB);
    internal abstract ContactInfo? IntersectsPolygon(Vector2 posA, float rotA, PolygonShape polygon, Vector2 posB, float rotB);
    internal abstract ContactInfo? IntersectsAABB   (Vector2 posA, float rotA, AABBShape    aabb,    Vector2 posB);
}
