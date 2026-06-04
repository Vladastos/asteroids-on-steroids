using System.Numerics;
using AsteroidsEngine.Engine.Components;
using AsteroidsEngine.Engine.Core;
using AsteroidsEngine.Engine.Events;

namespace AsteroidsEngine.Engine.Destruction;

/// <summary>A cell vaporised mid-propagation — the game turns this into dust.</summary>
public readonly struct CellPulverizedEvent
{
    public readonly Entity Body;
    public readonly Vector2 WorldCentroid;
    public readonly float Area;
    public CellPulverizedEvent(Entity body, Vector2 worldCentroid, float area)
    { Body = body; WorldCentroid = worldCentroid; Area = area; }
}

/// <summary>A multi-frame fracture finished; the body should be replaced by these
/// fragments (pulverised cells were already vaporised live, so they're not included).</summary>
public readonly struct FractureCompletedEvent
{
    public readonly Entity Body;
    public readonly FragmentSpec[] Fragments;
    public FractureCompletedEvent(Entity body, FragmentSpec[] fragments)
    { Body = body; Fragments = fragments; }
}

/// <summary>
/// Advances every live <see cref="FractureProcess"/>: each iteration steps its fronts a
/// few frontier-pops (co-propagating through the shared broken-bond state), vaporises
/// cells whose crack energy crossed the blast threshold (→ CellPulverizedEvent), and when
/// all fronts are spent finalises the body into fragments (→ FractureCompletedEvent). The
/// game wires the events (dust VFX, spawning fragments, destroying the original).
/// </summary>
public sealed class FractureCrackSystem : ISystem
{
    private readonly EventBus _bus;
    private readonly Random _rng;
    private readonly List<Entity> _scratch = new();

    public FractureCrackSystem(EventBus bus, Random rng) { _bus = bus; _rng = rng; }

    public void Update(World world, double dt)
    {
        _scratch.Clear();
        _scratch.AddRange(world.QueryEntities<FractureProcess>());

        foreach (var e in _scratch)
        {
            if (!world.IsAlive(e) || !world.HasComponent<FractureProcess>(e)) continue;
            ref var fp = ref world.GetComponent<FractureProcess>(e);
            if (fp.Done) continue;

            if (++fp.FrameCounter < fp.FramesPerIteration) continue;
            fp.FrameCounter = 0;

            ref var body = ref world.GetComponent<FracturableBody>(e);

            // --- advance the fronts, co-propagating through the shared Broken[] ---
            int steps = fp.StepsPerIteration < 1 ? 1 : fp.StepsPerIteration;
            for (int s = 0; s < steps; s++)
            {
                bool any = false;
                foreach (var f in fp.Fronts)
                {
                    if (!f.Active) continue;
                    FractureKernel.StepFront(f, body.Cells, body.Bonds, fp.Adj, fp.Eff, fp.Broken);
                    any = true;
                }
                if (!any) break;
            }

            // --- vaporise cells whose crack energy crossed the blast threshold ---
            ref var t = ref world.GetComponent<Transform>(e);
            float cos = MathF.Cos(t.Rotation), sin = MathF.Sin(t.Rotation);
            foreach (var f in fp.Fronts)
            {
                float[] en = f.Energy;
                for (int i = 0; i < body.Cells.Length; i++)
                {
                    if (fp.Pulverized[i] || en[i] <= 0f || en[i] <= f.BlastThresh) continue;
                    fp.Pulverized[i] = true;
                    Vector2 lc = body.Cells[i].Centroid;
                    Vector2 wc = new(lc.X * cos - lc.Y * sin + t.Position.X,
                                     lc.X * sin + lc.Y * cos + t.Position.Y);
                    _bus.Publish(new CellPulverizedEvent(e, wc, body.Cells[i].Area));
                }
            }

            // --- finalise once every front is spent ---
            bool active = false;
            foreach (var f in fp.Fronts) if (f.Active) { active = true; break; }
            if (active) continue;

            var fragments = Finalize(world, e, in fp, in body, in t);
            fp.Done = true;
            _bus.Publish(new FractureCompletedEvent(e, fragments));
        }
    }

    private FragmentSpec[] Finalize(World world, Entity e, in FractureProcess fp, in FracturableBody body, in Transform t)
    {
        Vector2 lin = Vector2.Zero; float ang = 0f;
        if (world.HasComponent<Velocity>(e))
        {
            ref var v = ref world.GetComponent<Velocity>(e);
            lin = v.Linear; ang = v.Angular;
        }
        float mass = world.HasComponent<RigidBody>(e) ? world.GetComponent<RigidBody>(e).Mass : 1f;

        var input = new FractureInput
        {
            ImpactPointWorld = fp.ImpactPointWorld,
            ImpactDir = fp.ImpactDir,
            MomentumKick = fp.MomentumKick,
            EjectSpeed = fp.EjectSpeed,
            ImpactSpin = fp.ImpactSpin,
            Directionality = fp.Directionality,
            BodyPosition = t.Position,
            BodyRotation = t.Rotation,
            BodyLinear = lin,
            BodyAngular = ang,
            BodyMass = mass,
        };
        return FractureSimulator.BuildResult(body, input, fp.Broken, fp.Pulverized, _rng);
    }
}
