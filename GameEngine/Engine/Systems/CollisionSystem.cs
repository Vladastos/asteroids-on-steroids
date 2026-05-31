using System.Numerics;
using AsteroidsEngine.Engine.Collision;
using AsteroidsEngine.Engine.Components;
using AsteroidsEngine.Engine.Core;
using AsteroidsEngine.Engine.Events;

namespace AsteroidsEngine.Engine.Systems;

/// <summary>
/// Detects collisions between entities with Transform + Collider components.
///
/// Each frame:
///   1. Rebuild spatial index (insert all collidable entity AABBs)
///   2. For each entity, query candidates from the index
///   3. Run narrow-phase (shape.Intersects) on each candidate pair
///   4. Publish CollisionEvent (deferred) for each confirmed contact
///   5. Optionally resolve overlap (separate overlapping rigid bodies)
///
/// A pair (A, B) is tested only if (A.Mask & B.Layer) != 0.
/// Each pair is tested at most once per frame.
/// </summary>
public sealed class CollisionSystem : ISystem
{
    private readonly ISpatialIndex _spatial;
    private readonly EventBus      _bus;

    // Reused per-frame buffers to avoid allocations.
    private readonly List<Entity>        _candidates  = new();
    private readonly HashSet<(int, int)> _testedPairs = new();

    public bool ResolveOverlap { get; set; } = true;

    public CollisionSystem(ISpatialIndex spatial, EventBus bus)
    {
        _spatial = spatial;
        _bus     = bus;
    }

    public void Update(World world, double dt)
    {
        _spatial.Clear();
        _testedPairs.Clear();

        // --- Phase 1: populate spatial index ---
        world.ForEach<Transform, Collider>((Entity e, ref Transform t, ref Collider c) =>
        {
            if (world.HasComponent<DisabledTag>(e)) return;
            var (min, max) = c.Shape.GetAABB(t.Position, t.Rotation);
            _spatial.Insert(e, min, max);
        });

        // --- Phase 2 & 3: query + narrow phase ---
        world.ForEach<Transform, Collider>((Entity entityA, ref Transform tA, ref Collider cA) =>
        {
            if (world.HasComponent<DisabledTag>(entityA)) return;

            var (min, max) = cA.Shape.GetAABB(tA.Position, tA.Rotation);
            _candidates.Clear();
            _spatial.GetCandidates(min, max, _candidates);

            foreach (var entityB in _candidates)
            {
                if (entityB == entityA) continue;

                // Canonical pair ordering: lower ID first — prevents A-B and B-A both being tested.
                int idA = entityA.Id, idB = entityB.Id;
                var pair = idA < idB ? (idA, idB) : (idB, idA);
                if (!_testedPairs.Add(pair)) continue;

                if (!world.IsAlive(entityB)) continue;
                ref var cB = ref world.GetComponent<Collider>(entityB);

                // Layer / mask filter.
                if ((cA.Mask & cB.Layer) == 0 && (cB.Mask & cA.Layer) == 0) continue;

                ref var tB = ref world.GetComponent<Transform>(entityB);

                var contact = cA.Shape.Intersects(tA.Position, tA.Rotation,
                                                   cB.Shape,
                                                   tB.Position, tB.Rotation);
                if (contact == null) continue;

                // Resolve overlap (push apart) if both have RigidBodies.
                if (ResolveOverlap)
                {
                    TrySeparate    (world, entityA, ref tA, entityB, ref tB, contact.Value);
                    TryApplyImpulse(world, entityA, entityB, contact.Value);
                }

                _bus.Publish(new CollisionEvent(entityA, entityB, contact.Value));
            }
        });
    }

    /// <summary>
    /// Pushes overlapping entities apart along the contact normal,
    /// weighted by their masses. Skipped if either entity has no RigidBody.
    /// </summary>
    private static void TrySeparate(World world,
                                     Entity eA, ref Transform tA,
                                     Entity eB, ref Transform tB,
                                     ContactInfo contact)
    {
        bool hasA = world.HasComponent<RigidBody>(eA);
        bool hasB = world.HasComponent<RigidBody>(eB);
        if (!hasA || !hasB) return;

        float massA = world.GetComponent<RigidBody>(eA).Mass;
        float massB = world.GetComponent<RigidBody>(eB).Mass;
        float total  = massA + massB;
        float shareA = total > 0f ? massB / total : 0.5f;

        var correction = contact.Normal * contact.Depth;
        tA.Position += correction *  shareA;
        tB.Position -= correction * (1f - shareA);
    }

    /// <summary>
    /// Applies a velocity impulse to resolve the collision, including both linear
    /// and angular components (full 2D rigid-body formula).
    ///
    /// j = −(1 + e) × v_rel_n
    ///     ────────────────────────────────────────────────
    ///     1/mA + 1/mB + (rA×n)²/IA + (rB×n)²/IB
    ///
    /// Skipped if either entity has no RigidBody. Uses min(eA.Restitution, eB.Restitution).
    /// </summary>
    private static void TryApplyImpulse(World world, Entity eA, Entity eB, ContactInfo contact)
    {
        if (!world.HasComponent<RigidBody>(eA) || !world.HasComponent<RigidBody>(eB)) return;
        if (!world.HasComponent<Velocity>(eA)  || !world.HasComponent<Velocity>(eB))  return;

        ref var rbA = ref world.GetComponent<RigidBody>(eA);
        ref var rbB = ref world.GetComponent<RigidBody>(eB);
        ref var vA  = ref world.GetComponent<Velocity>(eA);
        ref var vB  = ref world.GetComponent<Velocity>(eB);
        ref var tA  = ref world.GetComponent<Transform>(eA);
        ref var tB  = ref world.GetComponent<Transform>(eB);

        Vector2 n  = contact.Normal;
        Vector2 rA = contact.ContactPoint - tA.Position;
        Vector2 rB = contact.ContactPoint - tB.Position;

        // Velocity at the contact point, including rotation (ω × r in 2D = (-ω·r.Y, ω·r.X)).
        Vector2 vContactA = vA.Linear + new Vector2(-vA.Angular * rA.Y,  vA.Angular * rA.X);
        Vector2 vContactB = vB.Linear + new Vector2(-vB.Angular * rB.Y,  vB.Angular * rB.X);
        float   vRelN     = Vector2.Dot(vContactA - vContactB, n);

        // Already separating — no impulse needed.
        if (vRelN >= 0f) return;

        float e = MathF.Min(rbA.Restitution, rbB.Restitution);

        // 2D cross product r × n (scalar).
        float rAn = rA.X * n.Y - rA.Y * n.X;
        float rBn = rB.X * n.Y - rB.Y * n.X;

        float denom = 1f / rbA.Mass + 1f / rbB.Mass;
        if (rbA.Inertia > 0f) denom += rAn * rAn / rbA.Inertia;
        if (rbB.Inertia > 0f) denom += rBn * rBn / rbB.Inertia;
        if (denom <= 0f) return;

        float j = -(1f + e) * vRelN / denom;

        Vector2 impulse = j * n;
        vA.Linear  +=  impulse / rbA.Mass;
        vB.Linear  -=  impulse / rbB.Mass;

        if (rbA.Inertia > 0f) vA.Angular += (rA.X * impulse.Y - rA.Y * impulse.X) / rbA.Inertia;
        if (rbB.Inertia > 0f) vB.Angular -= (rB.X * impulse.Y - rB.Y * impulse.X) / rbB.Inertia;
    }
}
