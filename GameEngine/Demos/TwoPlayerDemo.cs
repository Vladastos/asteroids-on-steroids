// TwoPlayerDemo.cs — two players + physics balls, fully ECS-driven.
//
// Compile: bash compile-two-player.sh
// Run:     mono TwoPlayerDemo.exe
//
// Controls:
//   P1 (blue)  — WASD
//   P2 (red)   — Arrow keys
//   R          — clear all balls

using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Numerics;
using System.Threading;
using System.Windows.Forms;
using AsteroidsEngine.Engine.Collision;
using AsteroidsEngine.Engine.Components;
using AsteroidsEngine.Engine.Core;
using AsteroidsEngine.Engine.Events;
using AsteroidsEngine.Engine.Input;
using AsteroidsEngine.Engine.Rendering;
using AsteroidsEngine.Engine.Systems;

// =============================================================================
// Demo-specific components — stored in World SparseSets like any engine component
// =============================================================================

struct PlayerTag { public int Index; }   // 1 = P1, 2 = P2

struct BallTag { }                        // zero-size marker; used as a query filter

struct CircleVisual                       // rendering data — replaces Sprite for this demo
{
    public Color  Fill;
    public bool   ShowOutline;
    public string Label;                  // null = no label drawn
}

// =============================================================================
// Collision layer bitmasks
// =============================================================================
static class Layers
{
    public const int Player = 1;
    public const int Ball   = 2;
}

// =============================================================================
// PlayerInputSystem — reads InputSystem, applies thrust forces to player entities
// =============================================================================
class PlayerInputSystem : ISystem
{
    private readonly InputSystem _input;
    private readonly Entity      _p1, _p2;
    private const float Thrust = 1800f;   // N — divided by mass in PhysicsSystem

    public PlayerInputSystem(InputSystem input, Entity p1, Entity p2)
    {
        _input = input; _p1 = p1; _p2 = p2;
    }

    public void Update(World world, double dt)
    {
        Thrust2D(world, _p1, KeyCode.W,  KeyCode.S,    KeyCode.A,    KeyCode.D);
        Thrust2D(world, _p2, KeyCode.Up, KeyCode.Down, KeyCode.Left, KeyCode.Right);

        // 'R' defers destruction of all ball entities — safe during update
        if (_input.IsPressed(KeyCode.R))
            foreach (var e in world.QueryEntities<BallTag>())
                world.DestroyEntity(e);
    }

    private void Thrust2D(World world, Entity e,
                           KeyCode up, KeyCode down, KeyCode left, KeyCode right)
    {
        float ax = 0, ay = 0;
        if (_input.IsHeld(up))    ay -= Thrust;
        if (_input.IsHeld(down))  ay += Thrust;
        if (_input.IsHeld(left))  ax -= Thrust;
        if (_input.IsHeld(right)) ax += Thrust;
        PhysicsSystem.ApplyForce(world, e, new Vector2(ax, ay));
    }
}

// =============================================================================
// WallBounceSystem — reflects balls off the world boundary; clamps players
// =============================================================================
class WallBounceSystem : ISystem
{
    private readonly int _w, _h;
    public WallBounceSystem(int w, int h) { _w = w; _h = h; }

    public void Update(World world, double dt)
    {
        int w = _w, h = _h;

        world.ForEach<Transform, Velocity, BallTag>(
            (Entity _, ref Transform t, ref Velocity v, ref BallTag _b) =>
        {
            const float r = 12f;
            if (t.Position.X < r)     { t.Position = new Vector2(r,     t.Position.Y); v.Linear = new Vector2(MathF.Abs(v.Linear.X),  v.Linear.Y); }
            if (t.Position.X > w - r) { t.Position = new Vector2(w - r, t.Position.Y); v.Linear = new Vector2(-MathF.Abs(v.Linear.X), v.Linear.Y); }
            if (t.Position.Y < r)     { t.Position = new Vector2(t.Position.X, r);     v.Linear = new Vector2(v.Linear.X,  MathF.Abs(v.Linear.Y)); }
            if (t.Position.Y > h - r) { t.Position = new Vector2(t.Position.X, h - r); v.Linear = new Vector2(v.Linear.X, -MathF.Abs(v.Linear.Y)); }
        });

        world.ForEach<Transform, Velocity, PlayerTag>(
            (Entity _, ref Transform t, ref Velocity v, ref PlayerTag _p) =>
        {
            const float r = 22f;
            if (t.Position.X < r)     { t.Position = new Vector2(r,     t.Position.Y); if (v.Linear.X < 0) v.Linear = new Vector2(0, v.Linear.Y); }
            if (t.Position.X > w - r) { t.Position = new Vector2(w - r, t.Position.Y); if (v.Linear.X > 0) v.Linear = new Vector2(0, v.Linear.Y); }
            if (t.Position.Y < r)     { t.Position = new Vector2(t.Position.X, r);     if (v.Linear.Y < 0) v.Linear = new Vector2(v.Linear.X, 0); }
            if (t.Position.Y > h - r) { t.Position = new Vector2(t.Position.X, h - r); if (v.Linear.Y > 0) v.Linear = new Vector2(v.Linear.X, 0); }
        });
    }
}

// =============================================================================
// BallSpawnSystem — creates ball entities from the centre on a timer
// =============================================================================
class BallSpawnSystem : ISystem
{
    private const float Interval = 1.5f;
    private const int   MaxBalls = 25;

    public float SpawnProgress { get; private set; }   // 0–1, read by CircleDrawSystem

    private float  _timer;
    private readonly int    _cx, _cy;
    private readonly Random _rng = new Random();

    private static readonly Color[] Palette =
    {
        Color.HotPink, Color.LimeGreen, Color.Gold,
        Color.Cyan, Color.OrangeRed, Color.MediumPurple, Color.Aquamarine,
    };

    public BallSpawnSystem(int worldW, int worldH)
    {
        _cx = worldW / 2;
        _cy = worldH / 2;
    }

    public void Update(World world, double dt)
    {
        _timer += (float)dt;
        SpawnProgress = Math.Min(_timer / Interval, 1f);

        if (_timer < Interval) return;
        _timer = 0f;
        if (world.Count<BallTag>() >= MaxBalls) return;

        float angle = (float)(_rng.NextDouble() * Math.PI * 2);
        float speed = 120f + (float)(_rng.NextDouble() * 350f);
        Color col   = Palette[_rng.Next(Palette.Length)];

        Entity ball = world.CreateEntity();
        world.AddComponent(ball, new Transform { Position = new Vector2(_cx, _cy) });
        world.AddComponent(ball, new Velocity  { Linear = new Vector2(MathF.Cos(angle), MathF.Sin(angle)) * speed });
        world.AddComponent(ball, new RigidBody { Mass = 0.4f, LinearDrag = 0.25f });
        world.AddComponent(ball, new Collider  { Shape = new CircleShape(12f), Layer = Layers.Ball, Mask = Layers.Player | Layers.Ball });
        world.AddComponent(ball, new BallTag());
        world.AddComponent(ball, new CircleVisual { Fill = col, ShowOutline = false });
    }
}

// =============================================================================
// EventFlushSystem — dispatches queued events after all other systems have run
// =============================================================================
class EventFlushSystem : ISystem
{
    private readonly EventBus _bus;
    public EventFlushSystem(EventBus bus) { _bus = bus; }
    public void Update(World world, double dt) => _bus.Flush();
}

// =============================================================================
// CircleDrawSystem — renders all Transform+Collider+CircleVisual entities
// =============================================================================
class CircleDrawSystem : IDrawSystem
{
    private readonly Camera          _camera;
    private readonly BallSpawnSystem _spawner;
    private readonly InputSystem     _input;
    private readonly int             _w, _h;

    private static readonly Font HudFont   = new Font("Consolas", 13f, FontStyle.Bold);
    private static readonly Font LabelFont = new Font("Consolas", 9f,  FontStyle.Bold);
    private static readonly Font KeyFont   = new Font("Consolas", 11f);

    public CircleDrawSystem(Camera camera, BallSpawnSystem spawner, InputSystem input, int w, int h)
    {
        _camera = camera; _spawner = spawner; _input = input; _w = w; _h = h;
    }

    public void Draw(World world, Graphics g)
    {
        g.SmoothingMode = SmoothingMode.AntiAlias;
        g.Clear(Color.FromArgb(12, 12, 20));

        DrawGrid(g);
        DrawSpawnRing(g);

        _camera.ApplyTo(g);

        world.ForEach<Transform, Collider, CircleVisual>(
            (Entity _, ref Transform t, ref Collider c, ref CircleVisual cv) =>
        {
            if (c.Shape is CircleShape circle)
                DrawDisc(g, t.Position.X, t.Position.Y, circle.Radius,
                         cv.Fill, cv.ShowOutline, cv.Label);
        });

        _camera.ResetTransform(g);

        // HUD
        int balls = world.Count<BallTag>();
        string info = string.Format("Balls: {0} / 25   [R] clear", balls);
        g.DrawString(info, HudFont, Brushes.DimGray,
            (_w - g.MeasureString(info, HudFont).Width) / 2f, 10f);

        DrawKeys(g, 20,       _h - 86, KeyCode.W,  KeyCode.A,    KeyCode.S,    KeyCode.D,     Color.FromArgb(80, 140, 255));
        DrawKeys(g, _w - 106, _h - 86, KeyCode.Up, KeyCode.Left, KeyCode.Down, KeyCode.Right, Color.FromArgb(255, 80, 70));
    }

    private void DrawGrid(Graphics g)
    {
        using (var pen = new Pen(Color.FromArgb(22, 255, 255, 255)))
        {
            for (int x = 0; x <= _w; x += 80) g.DrawLine(pen, x, 0, x, _h);
            for (int y = 0; y <= _h; y += 80) g.DrawLine(pen, 0, y, _w, y);
        }
    }

    private void DrawSpawnRing(Graphics g)
    {
        float t = _spawner.SpawnProgress;
        float r = 8f + 24f * t;
        int   a = (int)(100 * t);
        using (var pen = new Pen(Color.FromArgb(a, 255, 240, 80), 2f))
            g.DrawEllipse(pen, _w / 2f - r, _h / 2f - r, r * 2, r * 2);
    }

    private void DrawDisc(Graphics g, float x, float y, float r,
                          Color fill, bool outline, string label)
    {
        using (var glow = new SolidBrush(Color.FromArgb(35, fill.R, fill.G, fill.B)))
        { float gr = r + 9f; g.FillEllipse(glow, x - gr, y - gr, gr * 2, gr * 2); }

        using (var br = new SolidBrush(fill))
            g.FillEllipse(br, x - r, y - r, r * 2, r * 2);

        if (outline)
            using (var pen = new Pen(Color.White, 2f))
                g.DrawEllipse(pen, x - r, y - r, r * 2, r * 2);

        if (label != null)
        {
            SizeF sz = g.MeasureString(label, LabelFont);
            g.DrawString(label, LabelFont, Brushes.White,
                x - sz.Width / 2f, y - sz.Height / 2f);
        }
    }

    private void DrawKeys(Graphics g, int x, int y,
        KeyCode up, KeyCode left, KeyCode down, KeyCode right, Color accent)
    {
        DrawKey(g, x + 28, y,      "^", _input.IsHeld(up),    accent);
        DrawKey(g, x,      y + 28, "<", _input.IsHeld(left),  accent);
        DrawKey(g, x + 28, y + 28, "v", _input.IsHeld(down),  accent);
        DrawKey(g, x + 56, y + 28, ">", _input.IsHeld(right), accent);
    }

    private void DrawKey(Graphics g, int x, int y, string lbl, bool active, Color accent)
    {
        using (var fill = new SolidBrush(active ? accent : Color.FromArgb(55, 255, 255, 255)))
            g.FillRectangle(fill, x, y, 24, 24);
        using (var border = new Pen(Color.FromArgb(100, 255, 255, 255), 1f))
            g.DrawRectangle(border, x, y, 24, 24);
        SizeF sz = g.MeasureString(lbl, KeyFont);
        g.DrawString(lbl, KeyFont, active ? Brushes.White : Brushes.Gray,
            x + (24 - sz.Width) / 2f, y + (24 - sz.Height) / 2f);
    }
}

// =============================================================================
// DemoWindow — WinForms shell: owns bitmaps, wires input, hosts the engine loop
// =============================================================================
class DemoWindow : Form
{
    private const int   W           = 1280;
    private const int   H           = 720;
    private const float Restitution = 0.85f;

    private Bitmap _back, _front;

    public DemoWindow()
    {
        Text            = "Two Player ECS  |  P1: WASD   P2: Arrows   R: clear";
        ClientSize      = new Size(W, H);
        FormBorderStyle = FormBorderStyle.FixedSingle;
        MaximizeBox     = false;
        StartPosition   = FormStartPosition.CenterScreen;
        KeyPreview      = true;
        SetStyle(ControlStyles.UserPaint | ControlStyles.AllPaintingInWmPaint, true);

        _back  = new Bitmap(W, H);
        _front = new Bitmap(W, H);
    }

    public void StartGame()
    {
        // --- Core engine objects ---
        var world  = new World();
        var input  = new InputSystem();
        var bus    = new EventBus();
        var camera = new Camera(W, H);
        var loop   = new GameLoop(world, input);

        // --- Wire WinForms events → InputSystem (UI thread) ---
        KeyDown     += (s, e) => { ((KeyEventArgs)e).Handled = true; input.OnKeyDown((KeyCode)(int)((KeyEventArgs)e).KeyCode); };
        KeyUp       += (s, e) => { ((KeyEventArgs)e).Handled = true; input.OnKeyUp  ((KeyCode)(int)((KeyEventArgs)e).KeyCode); };
        FormClosing += (s, e) => loop.Stop();

        // --- Player entities ---
        Entity p1 = SpawnPlayer(world, 1, new Vector2(W / 3f,     H / 2f), Color.FromArgb(80, 140, 255), "P1");
        Entity p2 = SpawnPlayer(world, 2, new Vector2(2 * W / 3f, H / 2f), Color.FromArgb(255, 80,  70), "P2");

        // --- Systems (execution order matters) ---
        var spawner = new BallSpawnSystem(W, H);
        var drawSys = new CircleDrawSystem(camera, spawner, input, W, H);

        loop.AddSystems(
            new PlayerInputSystem(input, p1, p2),  // 1. read input → accumulate forces
            new PhysicsSystem(),                    // 2. forces → velocity, apply drag
            new MovementSystem(),                   // 3. velocity → position
            new WallBounceSystem(W, H),             // 4. reflect/clamp at world edges
            spawner,                                // 5. spawn new balls if due
            new CollisionSystem(new SpatialGrid(128f), bus) { ResolveOverlap = true },
                                                    // 6. broad+narrow phase → separate positions, publish CollisionEvent
            new EventFlushSystem(bus)               // 7. flush bus → fire collision impulse response
        );

        // Elastic impulse response — subscribed before flush, fires in step 7
        bus.Subscribe<CollisionEvent>(ev => ApplyImpulse(world, ev));

        // --- Render callback: called each frame on the game thread ---
        loop.OnDraw = () =>
        {
            using (var g = Graphics.FromImage(_back))
                drawSys.Draw(world, g);

            var old = Interlocked.Exchange(ref _front, _back);
            _back = old;
            if (!IsDisposed) Invalidate();
        };

        loop.Start();
    }

    // -------------------------------------------------------------------------
    // Helpers
    // -------------------------------------------------------------------------

    private static Entity SpawnPlayer(World world, int index, Vector2 pos, Color col, string label)
    {
        Entity e = world.CreateEntity();
        world.AddComponent(e, new Transform  { Position = pos });
        world.AddComponent(e, new Velocity   ());
        world.AddComponent(e, new RigidBody  { Mass = 2f, LinearDrag = 2.5f });
        world.AddComponent(e, new Collider   { Shape = new CircleShape(22f), Layer = Layers.Player, Mask = Layers.Player | Layers.Ball });
        world.AddComponent(e, new PlayerTag  { Index = index });
        world.AddComponent(e, new CircleVisual { Fill = col, ShowOutline = true, Label = label });
        return e;
    }

    // Called from EventBus.Flush() (inside EventFlushSystem, on the game thread).
    // Applies an elastic velocity impulse along the contact normal.
    private static void ApplyImpulse(World world, CollisionEvent ev)
    {
        if (!world.IsAlive(ev.EntityA) || !world.IsAlive(ev.EntityB))               return;
        if (!world.HasComponent<Velocity>(ev.EntityA) ||
            !world.HasComponent<Velocity>(ev.EntityB))                               return;

        float mA = world.HasComponent<RigidBody>(ev.EntityA)
                       ? world.GetComponent<RigidBody>(ev.EntityA).Mass : 1f;
        float mB = world.HasComponent<RigidBody>(ev.EntityB)
                       ? world.GetComponent<RigidBody>(ev.EntityB).Mass : 1f;

        ref var vA = ref world.GetComponent<Velocity>(ev.EntityA);
        ref var vB = ref world.GetComponent<Velocity>(ev.EntityB);

        Vector2 n       = ev.Contact.Normal;         // from B toward A
        float   relVelN = Vector2.Dot(vA.Linear - vB.Linear, n);
        if (relVelN >= 0f) return;                   // already separating — skip

        float j = -(1f + Restitution) * relVelN / (1f / mA + 1f / mB);
        vA.Linear += n * (j / mA);
        vB.Linear -= n * (j / mB);
    }

    // -------------------------------------------------------------------------
    // WinForms paint — UI thread blits the last completed frame
    // -------------------------------------------------------------------------
    protected override void OnPaint(PaintEventArgs e)
    {
        var bmp = Volatile.Read(ref _front);
        if (bmp != null) e.Graphics.DrawImage(bmp, 0, 0);
    }

    protected override void OnPaintBackground(PaintEventArgs e) { }

    [STAThread]
    static void Main()
    {
        Application.EnableVisualStyles();
        Application.SetCompatibleTextRenderingDefault(false);
        var form = new DemoWindow();
        form.Show();
        form.Activate();
        form.StartGame();
        Application.Run(form);
    }
}
