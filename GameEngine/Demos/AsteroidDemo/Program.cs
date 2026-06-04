// Asteroids — Destruction Sandbox
//
//   WASD        thrust          Mouse      aim
//   Left-click  fire            R          respawn asteroids
//   Up/Down     select param    Left/Right adjust param
//   Tab         toggle panel    Esc        quit
//
//   cd GameEngine/Demos/AsteroidDemo && dotnet run

using System.Diagnostics;
using System.Numerics;
using System.Runtime.InteropServices;
using AsteroidsEngine.Engine.Collision;
using AsteroidsEngine.Engine.Components;
using AsteroidsEngine.Engine.Core;
using AsteroidsEngine.Engine.Destruction;
using AsteroidsEngine.Engine.Effects;
using AsteroidsEngine.Engine.Events;
using AsteroidsEngine.Engine.Input;
using AsteroidsEngine.Engine.Rendering;
using AsteroidsEngine.Engine.Systems;
using AsteroidsEngine.Platform.Sdl;

const int W = 1280, H = 800;

using var window = new SdlGameWindow("Asteroids — Destruction Sandbox", W, H);
var input = new InputSystem();
window.KeyDown += k => input.OnKeyDown(k);
window.KeyUp += k => input.OnKeyUp(k);
window.MouseMoved += p => input.OnMouseMove(p);
window.MouseButtonChanged += (b, pr) => input.OnMouseButton(b, pr);

var cfg = new Config();
var session = new DemoSession(W, H, input, cfg);
var renderer = new DemoRenderer(W, H);

const double FixedDt = 1.0 / 120.0;
var fixedStep = new FixedTimestep(FixedDt);
var sw = Stopwatch.StartNew();
long lastTicks = sw.ElapsedTicks;
bool showPanel = true;
double fps = 60.0;

while (!window.ShouldClose)
{
    window.PollEvents();

    long now = sw.ElapsedTicks;
    double frameTime = (double)(now - lastTicks) / Stopwatch.Frequency;
    lastTicks = now;
    if (frameTime > 0) fps += (1.0 / frameTime - fps) * 0.1;   // exponential smoothing

    input.BeginFrame();
    if (input.IsPressed(KeyCode.Escape)) break;

    // Panel / tuning input (once per render frame).
    if (input.IsPressed(KeyCode.Tab)) showPanel = !showPanel;
    if (input.IsPressed(KeyCode.Up)) cfg.T.Move(-1);
    if (input.IsPressed(KeyCode.Down)) cfg.T.Move(1);
    if (input.IsPressed(KeyCode.Left)) cfg.T.Adjust(-1);
    if (input.IsPressed(KeyCode.Right)) cfg.T.Adjust(1);
    if (input.IsPressed(KeyCode.R)) session.Respawn();
    if (input.IsPressed(KeyCode.M)) { cfg.CycleMaterial(); session.Respawn(); }

    int steps = fixedStep.Advance(frameTime);
    for (int i = 0; i < steps; i++) session.Update(FixedDt);

    renderer.Draw(window.Renderer, session, cfg, fixedStep.Alpha, showPanel, (float)fps);
    window.Present();

    double elapsed = (double)(sw.ElapsedTicks - now) / Stopwatch.Frequency;
    int sleep = (int)((1.0 / 120.0 - elapsed) * 1000);
    if (sleep > 1) Thread.Sleep(sleep);
}

// =============================================================================
// Constants, layers, components
// =============================================================================

static class GameConst { public const float PlayerRadius = 15f; }

static class Layers { public const int Asteroid = 1, Player = 2, Ghost = 4; }

struct PlayerTag { }
struct AsteroidTag { }
struct BulletTag { }
struct BulletVisual { public Color Color; }
struct TimeToLive { public float Remaining; }
struct AimComponent { public Vector2 Dir; }
struct ShootCooldown { public float Remaining; }
struct AsteroidColor { public Color Fill, Outline; }
// Fresh fragments don't collide for a short grace period so they separate without
// fighting each other (they spawn touching their siblings). On layer Ghost = no collision.
struct FractureGhost { public float Remaining; public bool Done; }
// Body-local boundary edges (pairs of points) — the silhouette + craters. Cached at
// spawn so the renderer can hide internal cell lines and stroke only the outline.
struct RenderOutline { public Vector2[] Outline; public Vector2[] Cracks; }

readonly struct BulletHitEvent
{
    public readonly Entity Asteroid, Bullet;
    public readonly int StruckCell;
    public readonly Vector2 Point, ShotDir;
    public BulletHitEvent(Entity asteroid, Entity bullet, int cell, Vector2 point, Vector2 shotDir)
    { Asteroid = asteroid; Bullet = bullet; StruckCell = cell; Point = point; ShotDir = shotDir; }
}

// =============================================================================
// DemoSession — owns the world, systems, spawning and the fracture handler
// =============================================================================

sealed class DemoSession
{
    private readonly World _world = new();
    private readonly EventBus _bus = new();
    private readonly ISystem[] _systems;
    private readonly Config _cfg;
    private readonly Random _rng = new(1234);
    private readonly ParticleSystem _fx = new();
    private readonly int _w, _h;
    private Entity _player;
    private float _respawnTimer = -1f;

    public World World => _world;
    public Entity Player => _player;
    public ParticleSystem Fx => _fx;

    public DemoSession(int w, int h, InputSystem input, Config cfg)
    {
        _w = w; _h = h; _cfg = cfg;

        SpawnPlayer(new Vector2(w / 2f, h / 2f));
        SpawnWave();

        _systems = new ISystem[]
        {
            new PreviousStateSystem(),
            new PlayerControlSystem(input, _player, cfg),
            new PhysicsSystem(),
            new MovementSystem(),
            new RaycastBulletSystem(_bus, _fx, _rng),
            new WrapSystem(w, h),
            new GhostSystem(),
            new CollisionSystem(new SpatialGrid(160f), _bus) { ResolveOverlap = true, EnableSleeping = false },
            new FractureCrackSystem(_bus, _rng),
            new EventFlushSystem(_bus),
            new TimeToLiveSystem(),
            new TunableApplySystem(cfg),
        };

        _bus.Subscribe<BulletHitEvent>(OnBulletHit);
        _bus.Subscribe<CellPulverizedEvent>(OnCellPulverized);
        _bus.Subscribe<FractureCompletedEvent>(OnFractureCompleted);
    }

    public void Update(double dt)
    {
        foreach (var s in _systems) s.Update(_world, dt);
        _world.FlushDeferred();
        _fx.Update((float)dt);

        // Auto-respawn when the field is cleared.
        if (_world.Count<AsteroidTag>() == 0)
        {
            if (_respawnTimer < 0f) _respawnTimer = 1.5f;
            else { _respawnTimer -= (float)dt; if (_respawnTimer <= 0f) { Respawn(); _respawnTimer = -1f; } }
        }
        else _respawnTimer = -1f;
    }

    public void Respawn()
    {
        foreach (var e in new List<Entity>(_world.QueryEntities<AsteroidTag>())) _world.DestroyEntity(e);
        SpawnWave();
    }

    // -------------------------------------------------------------------------

    private void SpawnPlayer(Vector2 pos)
    {
        _player = _world.CreateEntity();
        _world.AddComponent(_player, new Transform { Position = pos, PreviousPosition = pos });
        _world.AddComponent(_player, new Velocity());
        _world.AddComponent(_player, new RigidBody { Mass = 12f, Inertia = 0f, LinearDrag = 1.2f, AngularDrag = 2f, Restitution = 0.2f, Friction = 0.1f });
        _world.AddComponent(_player, new Collider { Shape = new CircleShape(GameConst.PlayerRadius), Layer = Layers.Player, Mask = Layers.Asteroid });
        _world.AddComponent(_player, new AimComponent { Dir = Vector2.UnitX });
        _world.AddComponent(_player, new ShootCooldown());
        _world.AddComponent(_player, new PlayerTag());
    }

    private void SpawnWave()
    {
        int count = (int)_cfg.AstCount.Value;
        for (int i = 0; i < count; i++)
        {
            Vector2 pos;
            do { pos = new Vector2(_rng.Next(_w), _rng.Next(_h)); }
            while ((pos - new Vector2(_w / 2f, _h / 2f)).Length() < 180f);   // not on top of the player
            SpawnAsteroid(pos);
        }
    }

    private void SpawnAsteroid(Vector2 pos)
    {
        var mat = new FractureProperties
        {
            Toughness = _cfg.Toughness.Value,
            Brittleness = _cfg.Brittleness.Value,
            GrainArea = _cfg.Grain.Value,
            MinFragmentArea = _cfg.MinFragArea.Value,
            Density = _cfg.Density.Value,
            KineticFraction = _cfg.KineticFraction.Value,
            SurfaceEfficiency = _cfg.SurfaceEff.Value,
            SpinPreStress = _cfg.SpinPreStress.Value,
        };
        var body = VoronoiTessellator.BuildAsteroid(_rng.Next(9, 14), _cfg.AstRadius.Value, mat, membership: null, _rng);

        float spread = (float)(_rng.NextDouble() * Math.PI * 2);
        var vel = new Vector2(MathF.Cos(spread), MathF.Sin(spread)) * (float)(_rng.NextDouble() * _cfg.AstSpeed.Value);
        float spin = (float)(_rng.NextDouble() * 2 - 1) * _cfg.AstSpin.Value;

        byte shade = (byte)_rng.Next(50, 80);
        SpawnBody(body, pos, (float)(_rng.NextDouble() * Math.PI * 2), vel, spin,
                  new AsteroidColor
                  {
                      Fill = new Color(shade, (byte)(shade - 6), (byte)(shade - 12)),
                      Outline = new Color(150, 138, 120)
                  });
    }

    private Entity SpawnBody(FracturableBody body, Vector2 pos, float rot, Vector2 vel, float spin, AsteroidColor color, bool ghost = false)
    {
        float area = VoronoiTessellator.TotalArea(body);
        float mass = MathF.Max(1f, body.Material.Density * area);
        float inertia = VoronoiTessellator.ComputeInertia(body, mass);

        var e = _world.CreateEntity();
        _world.AddComponent(e, new Transform { Position = pos, Rotation = rot, PreviousPosition = pos, PreviousRotation = rot });
        _world.AddComponent(e, new Velocity { Linear = vel, Angular = spin });
        _world.AddComponent(e, new RigidBody
        {
            Mass = mass,
            Inertia = inertia,
            LinearDrag = _cfg.LinDrag.Value,
            AngularDrag = _cfg.AngDrag.Value,
            Restitution = _cfg.Restitution.Value,
            Friction = _cfg.Friction.Value
        });
        _world.AddComponent(e, new Collider
        {
            Shape = VoronoiTessellator.BuildShape(body),
            Layer = ghost ? Layers.Ghost : Layers.Asteroid,
            Mask = ghost ? 0 : (Layers.Asteroid | Layers.Player)
        });
        _world.AddComponent(e, body);
        _world.AddComponent(e, new AsteroidTag());
        _world.AddComponent(e, color);
        var (outline, cracks) = ComputeEdges(body.Cells, body.Bonds);
        _world.AddComponent(e, new RenderOutline { Outline = outline, Cracks = cracks });
        if (ghost) _world.AddComponent(e, new FractureGhost { Remaining = 0.0f });
        return e;
    }

    /// <summary>Boundary edges of a celled body: a cell edge whose midpoint isn't
    /// shared with another cell (≈ unshared edge). Returns body-local segment endpoint
    /// pairs — the outer silhouette plus any crater edges left by vaporised cells.</summary>
    /// <summary>Classifies a body's cell edges for rendering: OUTLINE (unshared
    /// silhouette + crater edges) and CRACKS (edges shared by two cells no longer
    /// bonded). Both are body-local segment-endpoint pairs.</summary>
    private static (Vector2[] outline, Vector2[] cracks) ComputeEdges(Cell[] cells, Bond[] bonds)
    {
        var bonded = new HashSet<(int, int)>();
        foreach (var b in bonds) bonded.Add((Math.Min(b.A, b.B), Math.Max(b.A, b.B)));

        // edge-midpoint key → the up-to-two cells that own that edge
        var edgeCells = new Dictionary<(int, int), (int a, int b)>();
        for (int ci = 0; ci < cells.Length; ci++)
        {
            var v = cells[ci].Local; int n = v.Length;
            for (int i = 0; i < n; i++)
            {
                Vector2 mid = (v[i] + v[(i + 1) % n]) * 0.5f;
                var key = ((int)MathF.Round(mid.X * 2f), (int)MathF.Round(mid.Y * 2f));
                edgeCells[key] = edgeCells.TryGetValue(key, out var p) ? (p.a, ci) : (ci, -1);
            }
        }

        var outline = new List<Vector2>();
        var cracks = new List<Vector2>();
        for (int ci = 0; ci < cells.Length; ci++)
        {
            var v = cells[ci].Local; int n = v.Length;
            for (int i = 0; i < n; i++)
            {
                Vector2 a = v[i], b = v[(i + 1) % n];
                Vector2 mid = (a + b) * 0.5f;
                var key = ((int)MathF.Round(mid.X * 2f), (int)MathF.Round(mid.Y * 2f));
                var (c0, c1) = edgeCells[key];
                if (c1 < 0) { outline.Add(a); outline.Add(b); }                       // unshared → silhouette
                else if (!bonded.Contains((Math.Min(c0, c1), Math.Max(c0, c1))))      // shared but not bonded → crack
                {
                    if (ci == Math.Min(c0, c1)) { cracks.Add(a); cracks.Add(b); }     // dedupe (edge is in both cells)
                }
                // else bonded shared edge → hidden
            }
        }
        return (outline.ToArray(), cracks.ToArray());
    }

    // -------------------------------------------------------------------------
    // Fracture handler
    // -------------------------------------------------------------------------

    private void OnBulletHit(BulletHitEvent ev)
    {
        if (!_world.IsAlive(ev.Asteroid) || !_world.IsAlive(ev.Bullet)) return;

        Vector2 bulletVel = _world.GetComponent<Velocity>(ev.Bullet).Linear;
        _world.DestroyEntity(ev.Bullet);

        float bulletMass = _cfg.BulletMass.Value * _cfg.EnergyScale.Value;   // EnergyScale folds into mass (E ∝ m)
        var weapon = new WeaponProfile
        {
            Directionality = _cfg.Directionality.Value, MomentumTransfer = _cfg.MomentumTransfer.Value,
            EjectFraction = _cfg.EjectFraction.Value, ImpactSpin = _cfg.ImpactSpin.Value,
            BlastFraction = _cfg.Blast.Value,
        };
        var timing = new FractureTiming
        {
            StepsPerIteration = (int)_cfg.CrackSteps.Value,
            FramesPerIteration = (int)_cfg.CrackFrames.Value,
        };

        // Impact flash at the hit (energy proxy ∝ bullet KE).
        EmitFlash(ev.Point, 0.5f * bulletMass * bulletVel.LengthSquared());

        // Seed (or extend) a multi-frame crack. Dust and fragments arrive over the next
        // frames via CellPulverizedEvent / FractureCompletedEvent.
        FractureService.BeginFracture(
            _world, ev.Asteroid, ev.StruckCell, ev.Point, ev.ShotDir,
            bulletVel, bulletMass, weapon, timing, _rng);
    }

    private void OnCellPulverized(CellPulverizedEvent ev)
    {
        AsteroidColor color = BodyColor(ev.Body);
        Vector2 bodyPos = ev.WorldCentroid, carrier = Vector2.Zero;
        if (_world.IsAlive(ev.Body))
        {
            if (_world.HasComponent<Transform>(ev.Body)) bodyPos = _world.GetComponent<Transform>(ev.Body).Position;
            if (_world.HasComponent<Velocity>(ev.Body)) carrier = _world.GetComponent<Velocity>(ev.Body).Linear;
        }
        // Dust flies radially outward from the body centre, drifting with the body.
        EmitDustBurst(ev.WorldCentroid, ev.WorldCentroid - bodyPos, carrier, ev.Area, color, EnergyRef);
    }

    private void OnFractureCompleted(FractureCompletedEvent ev)
    {
        AsteroidColor color = BodyColor(ev.Body);
        foreach (var f in ev.Fragments)
        {
            if (f.IsDebris) { EmitDustBurst(f.WorldCentroid, f.Linear, Vector2.Zero, f.Area, color, EnergyRef); continue; }
            SpawnBody(f.Body, f.WorldCentroid, f.Rotation, f.Linear, f.Angular, color, ghost: true);
        }
        _world.DestroyEntity(ev.Body);
    }

    private static readonly AsteroidColor DefaultColor =
        new() { Fill = new Color(64, 58, 52), Outline = new Color(150, 138, 120) };

    private AsteroidColor BodyColor(Entity e) =>
        _world.IsAlive(e) && _world.HasComponent<AsteroidColor>(e)
            ? _world.GetComponent<AsteroidColor>(e) : DefaultColor;

    // VFX modulation references (a default shot ≈ 1.0). Tuned to the default weapon.
    private const float EnergyRef = 80_000f;
    private const float DustAreaRef = 1400f;

    /// <summary>Impact flash: a bright disk that expands and fades; size ∝ impact energy.</summary>
    private void EmitFlash(Vector2 point, float energy)
    {
        if (_cfg.FlashSize.Value <= 0f) return;
        float e = Math.Clamp(energy / EnergyRef, 0.25f, 2.5f);
        float sz = _cfg.FlashSize.Value * e;
        float ttl = _cfg.FlashTtl.Value;
        _fx.Emit(new Particle
        {
            Position = point, Velocity = Vector2.Zero, Drag = 0f,
            Life = ttl, MaxLife = ttl,
            Size0 = sz * 0.35f, Size1 = sz,
            Color0 = new Color(255, 245, 210, 235), Color1 = new Color(255, 165, 70, 0),
        });
    }

    /// <summary>Dust burst (a vaporised cell or a tiny leftover shard). Count ∝ energy,
    /// flung in a cone around <paramref name="dirHint"/> and drifting with <paramref name="carrier"/>,
    /// size ∝ cell area, colour from the material.</summary>
    private void EmitDustBurst(Vector2 centroid, Vector2 dirHint, Vector2 carrier, float area, AsteroidColor color, float energy)
    {
        int n = (int)(_cfg.DustCount.Value * Math.Clamp(energy / EnergyRef, 0.35f, 2.5f));
        if (n <= 0) return;

        Vector2 vdir = dirHint.LengthSquared() > 1e-4f
            ? Vector2.Normalize(dirHint)
            : new Vector2(MathF.Cos((float)_rng.NextDouble() * MathF.Tau), MathF.Sin((float)_rng.NextDouble() * MathF.Tau));
        float cone   = _cfg.DustSpread.Value * MathF.PI;
        float baseSz = _cfg.DustSize.Value * MathF.Sqrt(MathF.Max(area, 1f) / DustAreaRef);
        Color dust   = color.Outline;

        for (int i = 0; i < n; i++)
        {
            float ang = ((float)_rng.NextDouble() * 2f - 1f) * cone;
            float ca = MathF.Cos(ang), sa = MathF.Sin(ang);
            Vector2 dir = new(vdir.X * ca - vdir.Y * sa, vdir.X * sa + vdir.Y * ca);
            float spd = _cfg.DustSpeed.Value * (0.4f + (float)_rng.NextDouble());
            float ttl = _cfg.DustTtl.Value * (0.6f + 0.6f * (float)_rng.NextDouble());
            float sz = baseSz * (0.7f + 0.6f * (float)_rng.NextDouble());
            Vector2 jit = new((float)_rng.NextDouble() - 0.5f, (float)_rng.NextDouble() - 0.5f);
            _fx.Emit(new Particle
            {
                Position = centroid + jit * baseSz, Velocity = dir * spd + carrier, Drag = 2.2f,
                Life = ttl, MaxLife = ttl,
                Size0 = sz, Size1 = sz * 0.1f,
                Color0 = dust.WithAlpha(220), Color1 = dust.WithAlpha(0),
            });
        }
    }
}

// =============================================================================
// Systems
// =============================================================================

sealed class PlayerControlSystem : ISystem
{
    private readonly InputSystem _input;
    private readonly Entity _player;
    private readonly Config _cfg;
    private static readonly Color BulletColor = new(255, 230, 90);

    public PlayerControlSystem(InputSystem input, Entity player, Config cfg)
    { _input = input; _player = player; _cfg = cfg; }

    public void Update(World world, double dt)
    {
        if (!world.IsAlive(_player)) return;

        Vector2 a = Vector2.Zero;
        if (_input.IsHeld(KeyCode.W)) a.Y -= 1; if (_input.IsHeld(KeyCode.S)) a.Y += 1;
        if (_input.IsHeld(KeyCode.A)) a.X -= 1; if (_input.IsHeld(KeyCode.D)) a.X += 1;
        if (a != Vector2.Zero)
            PhysicsSystem.ApplyForce(world, _player, Vector2.Normalize(a) * _cfg.Thrust.Value);

        ref var t = ref world.GetComponent<Transform>(_player);
        ref var aim = ref world.GetComponent<AimComponent>(_player);
        Vector2 toMouse = _input.MouseScreen - t.Position;
        if (toMouse.LengthSquared() > 1f) aim.Dir = Vector2.Normalize(toMouse);

        ref var cd = ref world.GetComponent<ShootCooldown>(_player);
        if (cd.Remaining > 0f) cd.Remaining -= (float)dt;

        if (_input.IsMouseLeft && cd.Remaining <= 0f)
        {
            cd.Remaining = _cfg.FireRate.Value;
            Vector2 muzzle = t.Position + aim.Dir * (GameConst.PlayerRadius + 6f);
            var b = world.CreateEntity();
            world.AddComponent(b, new Transform { Position = muzzle, PreviousPosition = muzzle });
            world.AddComponent(b, new Velocity { Linear = aim.Dir * _cfg.BulletSpeed.Value });
            world.AddComponent(b, new BulletTag());
            world.AddComponent(b, new BulletVisual { Color = BulletColor });
            world.AddComponent(b, new TimeToLive { Remaining = 1.5f });
        }
    }
}

// Sweeps each bullet's travel segment against asteroids (raycast → no tunnelling).
sealed class RaycastBulletSystem : ISystem
{
    private readonly EventBus _bus;
    private readonly ParticleSystem _fx;
    private readonly Random _rng;
    private readonly List<(Entity bullet, Vector2 from, Vector2 to)> _seg = new();

    public RaycastBulletSystem(EventBus bus, ParticleSystem fx, Random rng) { _bus = bus; _fx = fx; _rng = rng; }

    public void Update(World world, double dt)
    {
        _seg.Clear();
        world.ForEach<Transform, BulletTag>((Entity e, ref Transform t, ref BulletTag _) =>
            _seg.Add((e, t.PreviousPosition, t.Position)));

        foreach (var (bullet, from, to) in _seg)
        {
            if (!world.IsAlive(bullet)) continue;

            // Tracer sparks — a hot fleck shed along the bullet's path each step.
            float ttl = 0.08f + 0.06f * (float)_rng.NextDouble();
            _fx.Emit(new Particle
            {
                Position = to, Drag = 3f, Life = ttl, MaxLife = ttl,
                Velocity = new Vector2((float)_rng.NextDouble() - 0.5f, (float)_rng.NextDouble() - 0.5f) * 40f,
                Size0 = 1.7f, Size1 = 0.2f,
                Color0 = new Color(255, 235, 130, 210), Color1 = new Color(255, 110, 40, 0),
            });

            Vector2 d = to - from;
            if (d.LengthSquared() < 1e-4f) continue;
            if (PhysicsQueries.Raycast(world, from, to, Layers.Asteroid, out var hit))
                _bus.Publish(new BulletHitEvent(hit.Entity, bullet, hit.PartIndex, hit.Point, Vector2.Normalize(d)));
        }
    }
}

sealed class WrapSystem : ISystem
{
    private readonly float _w, _h;
    public WrapSystem(int w, int h) { _w = w; _h = h; }

    public void Update(World world, double dt)
    {
        world.ForEach<Transform>((Entity _, ref Transform t) =>
        {
            if (t.Position.X < 0) t.Position.X += _w; else if (t.Position.X > _w) t.Position.X -= _w;
            if (t.Position.Y < 0) t.Position.Y += _h; else if (t.Position.Y > _h) t.Position.Y -= _h;
        });
    }
}

sealed class TimeToLiveSystem : ISystem
{
    public void Update(World world, double dt)
    {
        var dead = new List<Entity>();
        foreach (var e in world.QueryEntities<TimeToLive>())
        {
            ref var ttl = ref world.GetComponent<TimeToLive>(e);
            ttl.Remaining -= (float)dt;
            if (ttl.Remaining <= 0f) dead.Add(e);
        }
        foreach (var e in dead) world.DestroyEntity(e);
    }
}

sealed class EventFlushSystem : ISystem
{
    private readonly EventBus _bus;
    public EventFlushSystem(EventBus bus) { _bus = bus; }
    public void Update(World world, double dt) => _bus.Flush();
}

// Counts down the spawn-grace on fresh fragments, then re-enables their collision.
sealed class GhostSystem : ISystem
{
    public void Update(World world, double dt)
    {
        float fdt = (float)dt;
        world.ForEach<FractureGhost, Collider>((Entity _, ref FractureGhost g, ref Collider c) =>
        {
            if (g.Done) return;
            g.Remaining -= fdt;
            if (g.Remaining <= 0f)
            {
                c.Layer = Layers.Asteroid;
                c.Mask = Layers.Asteroid | Layers.Player;
                g.Done = true;
            }
        });
    }
}

// Writes the live-tunable physics constants onto every body each frame.
sealed class TunableApplySystem : ISystem
{
    private readonly Config _cfg;
    public TunableApplySystem(Config cfg) { _cfg = cfg; }

    public void Update(World world, double dt)
    {
        var cfg = _cfg;
        world.ForEach<RigidBody>((Entity _, ref RigidBody rb) =>
        {
            rb.Restitution = cfg.Restitution.Value;
            rb.Friction = cfg.Friction.Value;
            rb.LinearDrag = cfg.LinDrag.Value;
            rb.AngularDrag = cfg.AngDrag.Value;
        });
        float tough = cfg.Toughness.Value;
        world.ForEach<FracturableBody>((Entity _, ref FracturableBody fb) =>
        {
            fb.Material.Brittleness = cfg.Brittleness.Value;
            fb.Material.KineticFraction = cfg.KineticFraction.Value;
            fb.Material.MinFragmentArea = cfg.MinFragArea.Value;
            fb.Material.SurfaceEfficiency = cfg.SurfaceEff.Value;
            fb.Material.SpinPreStress = cfg.SpinPreStress.Value;
            fb.Material.Toughness = tough;
            // Toughness is live: rescale every bond from its stored edge length.
            for (int i = 0; i < fb.Bonds.Length; i++)
                fb.Bonds[i].Strength = fb.Bonds[i].EdgeLength * tough;
        });
    }
}

// =============================================================================
// Renderer
// =============================================================================

sealed class DemoRenderer
{
    private readonly int _w, _h;
    private static readonly Color Bg = new(8, 9, 14);
    private static readonly Color PlayerFill = new(70, 130, 240);
    private static readonly Color PlayerEdge = new(170, 205, 255);
    private static readonly FontSpec Hud = new("monospace", 14f);
    private static readonly FontSpec Panel = new("monospace", 13f);
    private const float TeleSq = 200f * 200f;
    private readonly List<Vector2> _mesh = new();      // reused: all cell world-verts of a body
    private readonly List<int>     _meshLens = new();  // reused: per-cell vertex counts

    public DemoRenderer(int w, int h) { _w = w; _h = h; }

    public void Draw(IRenderer r, DemoSession session, Config cfg, float alpha, bool showPanel, float fps)
    {
        var world = session.World;
        r.Begin(Bg);

        // Asteroids — draw each cell.
        world.ForEach<Transform, FracturableBody, AsteroidColor>(
            (Entity e, ref Transform t, ref FracturableBody fb, ref AsteroidColor col) =>
        {
            var (pos, rot) = Interp(t, alpha);
            float c = MathF.Cos(rot), s = MathF.Sin(rot);

            // A body mid-fracture carries live damage masks; settled bodies use the cache.
            bool[]? broken = null, pulv = null;
            if (world.HasComponent<FractureProcess>(e))
            {
                ref var fp = ref world.GetComponent<FractureProcess>(e);
                broken = fp.Broken; pulv = fp.Pulverized;
            }

            // Fill cells as one path (skip vaporised cells → craters open live).
            _mesh.Clear(); _meshLens.Clear();
            for (int ci = 0; ci < fb.Cells.Length; ci++)
            {
                if (pulv != null && pulv[ci]) continue;
                var lv = fb.Cells[ci].Local;
                for (int k = 0; k < lv.Length; k++)
                    _mesh.Add(new Vector2(lv[k].X * c - lv[k].Y * s + pos.X, lv[k].X * s + lv[k].Y * c + pos.Y));
                _meshLens.Add(lv.Length);
            }
            r.FillPath(CollectionsMarshal.AsSpan(_mesh), CollectionsMarshal.AsSpan(_meshLens), col.Fill);

            // Outline (silhouette + craters) and cracks (broken bonds between touching cells).
            if (broken != null && pulv != null)
            {
                var (outline, cracks) = ComputeEdgesLive(fb.Cells, fb.Bonds, broken, pulv);
                DrawSegs(r, outline, pos, c, s, col.Outline, 1.5f);
                DrawSegs(r, cracks, pos, c, s, CrackColor(col.Fill), 1f);
            }
            else if (world.HasComponent<RenderOutline>(e))
            {
                var ro = world.GetComponent<RenderOutline>(e);
                DrawSegs(r, ro.Outline, pos, c, s, col.Outline, 1.5f);
                DrawSegs(r, ro.Cracks, pos, c, s, CrackColor(col.Fill), 1f);
            }
        });

        // Dust, sparks, flashes (drawn over the rock, under the bullets).
        session.Fx.Draw(r);

        // Bullets — a tapering tracer streak (glow + hot core) plus the round.
        float tracerLen = cfg.TracerLen.Value, tracerW = cfg.TracerWidth.Value;
        world.ForEach<Transform, BulletTag, BulletVisual>(
            (Entity _, ref Transform t, ref BulletTag _, ref BulletVisual bv) =>
        {
            var (p, _) = Interp(t, alpha);
            Vector2 d = t.Position - t.PreviousPosition;
            Vector2 dir = d.LengthSquared() > 1e-4f ? Vector2.Normalize(d) : new Vector2(0f, -1f);
            if (tracerLen > 0f)
            {
                Vector2 tail = p - dir * tracerLen;
                r.DrawLine(tail, p, new Color(255, 170, 60, 80), tracerW * 2.5f);    // glow
                r.DrawLine(tail, p, new Color(255, 240, 165, 220), tracerW);          // hot core
            }
            r.FillCircle(p, tracerW * 1.3f, bv.Color);                               // round
        });

        // Player.
        world.ForEach<Transform, PlayerTag, AimComponent>(
            (Entity _, ref Transform t, ref PlayerTag _, ref AimComponent aim) =>
        {
            var (p, _) = Interp(t, alpha);
            r.FillCircle(p, GameConst.PlayerRadius, PlayerFill);
            r.DrawCircle(p, GameConst.PlayerRadius, PlayerEdge, 2f);
            r.DrawLine(p, p + aim.Dir * (GameConst.PlayerRadius + 12f), PlayerEdge, 2f);
        });

        // HUD.
        r.DrawText($"fps {fps,3:F0}   asteroids {world.Count<AsteroidTag>()}   material {cfg.MaterialName}",
                   new Vector2(12, 10), new Color(190, 200, 220), Hud);
        r.DrawText("WASD move   mouse aim   click fire   R respawn   M material   arrows tune   Tab panel   Esc quit",
                   new Vector2(12, _h - 22f), new Color(110, 120, 140), Panel);

        if (showPanel) DrawPanel(r, cfg);

        r.End();
    }

    private void DrawPanel(IRenderer r, Config cfg)
    {
        var ps = cfg.T.Params;
        float x = _w - 270f, y = 8f, rowH = 18f;
        float panelH = ps.Count * rowH + 16f;

        Span<Vector2> bg = stackalloc Vector2[4]
        { new(x - 10, y - 4), new(_w - 4, y - 4), new(_w - 4, y + panelH), new(x - 10, y + panelH) };
        r.FillPolygon(bg, new Color(0, 0, 0, 150));

        for (int i = 0; i < ps.Count; i++)
        {
            var p = ps[i];
            if (p.IsHeader)
            {
                r.DrawText(p.Name, new Vector2(x, y + i * rowH), new Color(120, 200, 230), Panel);
                continue;
            }
            bool sel = i == cfg.T.Selected;
            Color c = sel ? new Color(255, 220, 110) : new Color(170, 178, 195);
            string line = $"{(sel ? ">" : " ")} {p.Name,-16} {p.Display}";
            r.DrawText(line, new Vector2(x, y + i * rowH), c, Panel);
        }
    }

    /// <summary>Edge classification for a body mid-fracture, from its live damage masks:
    /// vaporised cells are absent (their edges become crater rims), a shared edge is hidden
    /// only while its two cells are still bonded, otherwise it draws as a crack.</summary>
    private static (Vector2[] outline, Vector2[] cracks) ComputeEdgesLive(
        Cell[] cells, Bond[] bonds, bool[] broken, bool[] pulverized)
    {
        var bonded = new HashSet<(int, int)>();
        for (int bi = 0; bi < bonds.Length; bi++)
        {
            if (broken[bi]) continue;
            int a = bonds[bi].A, b = bonds[bi].B;
            if (pulverized[a] || pulverized[b]) continue;
            bonded.Add((Math.Min(a, b), Math.Max(a, b)));
        }

        var edgeCells = new Dictionary<(int, int), (int a, int b)>();
        for (int ci = 0; ci < cells.Length; ci++)
        {
            if (pulverized[ci]) continue;
            var v = cells[ci].Local; int n = v.Length;
            for (int i = 0; i < n; i++)
            {
                Vector2 mid = (v[i] + v[(i + 1) % n]) * 0.5f;
                var key = ((int)MathF.Round(mid.X * 2f), (int)MathF.Round(mid.Y * 2f));
                edgeCells[key] = edgeCells.TryGetValue(key, out var p) ? (p.a, ci) : (ci, -1);
            }
        }

        var outline = new List<Vector2>();
        var cracks = new List<Vector2>();
        for (int ci = 0; ci < cells.Length; ci++)
        {
            if (pulverized[ci]) continue;
            var v = cells[ci].Local; int n = v.Length;
            for (int i = 0; i < n; i++)
            {
                Vector2 a = v[i], b = v[(i + 1) % n];
                Vector2 mid = (a + b) * 0.5f;
                var key = ((int)MathF.Round(mid.X * 2f), (int)MathF.Round(mid.Y * 2f));
                var (c0, c1) = edgeCells[key];
                if (c1 < 0) { outline.Add(a); outline.Add(b); }
                else if (!bonded.Contains((Math.Min(c0, c1), Math.Max(c0, c1))))
                {
                    if (ci == Math.Min(c0, c1)) { cracks.Add(a); cracks.Add(b); }
                }
            }
        }
        return (outline.ToArray(), cracks.ToArray());
    }

    private static void DrawSegs(IRenderer r, Vector2[] segs, Vector2 pos, float c, float s, Color color, float w)
    {
        for (int k = 0; k + 1 < segs.Length; k += 2)
        {
            Vector2 a = segs[k], b = segs[k + 1];
            r.DrawLine(
                new Vector2(a.X * c - a.Y * s + pos.X, a.X * s + a.Y * c + pos.Y),
                new Vector2(b.X * c - b.Y * s + pos.X, b.X * s + b.Y * c + pos.Y),
                color, w);
        }
    }

    private static Color CrackColor(Color fill) =>
        new((byte)(fill.R * 0.35f), (byte)(fill.G * 0.35f), (byte)(fill.B * 0.35f));

    private static (Vector2 pos, float rot) Interp(in Transform t, float alpha)
    {
        Vector2 d = t.Position - t.PreviousPosition;
        if (d.LengthSquared() > TeleSq) return (t.Position, t.Rotation);
        float dr = t.Rotation - t.PreviousRotation;
        while (dr > MathF.PI) dr -= MathF.Tau;
        while (dr < -MathF.PI) dr += MathF.Tau;
        return (t.PreviousPosition + d * alpha, t.PreviousRotation + dr * alpha);
    }
}
