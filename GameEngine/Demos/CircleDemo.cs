// CircleDemo.cs — standalone smoke test for the engine's core concepts:
//   - Background-thread game loop with Stopwatch delta time
//   - Double-buffered bitmap rendering (back/front swap)
//   - Polling-based input (HashSet of held keys)
//   - Symplectic Euler physics (thrust + drag)
//
// Compile: mcs CircleDemo.cs -r:System.Windows.Forms.dll -r:System.Drawing.dll -out:CircleDemo.exe
// Run:     mono CircleDemo.exe
//
// NOTE: this file is excluded from dotnet build (see AsteroidsEngine.csproj <Compile Remove="Demos/**">).
// Always recompile with the mcs command above — dotnet build will NOT pick up your changes.

using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Threading;
using System.Windows.Forms;

class CircleDemo : Form
{
    // -------------------------------------------------------------------------
    // Window / buffer
    // -------------------------------------------------------------------------
    const int W = 1920, H = 1080;

    private Bitmap _back;
    private Bitmap _front;

    // -------------------------------------------------------------------------
    // Entity state  (single circle for now)
    // -------------------------------------------------------------------------
    private float _x, _y;         // world position (pixels)
    private float _vx, _vy;       // velocity (pixels/second)

    const float THRUST = 800f;  // acceleration when key is held
    const float DRAG = 1.5f;  // fraction of velocity lost per second (exponential)
    const float RADIUS = 24f;

    // -------------------------------------------------------------------------
    // Input  (UI thread writes, game thread reads)
    // -------------------------------------------------------------------------
    private readonly HashSet<Keys> _heldPending = new HashSet<Keys>();
    private readonly HashSet<Keys> _heldCommitted = new HashSet<Keys>();
    private readonly object _inputLock = new object();

    private volatile bool _running = true;

    // -------------------------------------------------------------------------
    // Constructor — WinForms setup
    // -------------------------------------------------------------------------
    public CircleDemo()
    {
        Text = "Circle Demo  |  WASD to move";
        ClientSize = new Size(W, H);
        FormBorderStyle = FormBorderStyle.FixedSingle;
        MaximizeBox = false;
        StartPosition = FormStartPosition.CenterScreen;

        // Suppress WinForms background erase — we cover every pixel ourselves.
        SetStyle(
            ControlStyles.UserPaint |
            ControlStyles.AllPaintingInWmPaint |
            ControlStyles.OptimizedDoubleBuffer,
            true);

        _back = new Bitmap(W, H);
        _front = new Bitmap(W, H);

        // Start position: screen centre
        _x = W / 2f;
        _y = H / 2f;

        // Ensure this form receives all key events regardless of which control has focus,
        // and prevent key-repeat events from firing duplicate KeyDown calls.
        KeyPreview = true;

        // Wire input events — run on UI thread, write to _heldPending
        KeyDown += (s, e) => { e.Handled = true; lock (_inputLock) _heldPending.Add(e.KeyCode); };
        KeyUp += (s, e) => { e.Handled = true; lock (_inputLock) _heldPending.Remove(e.KeyCode); };
        FormClosing += (s, e) => _running = false;
    }

    // -------------------------------------------------------------------------
    // Game loop — runs on background thread
    // -------------------------------------------------------------------------

    public void StartLoop()
    {
        var thread = new Thread(Loop) { IsBackground = true, Name = "GameLoop" };
        thread.Start();
    }

    private void Loop()
    {
        var sw = Stopwatch.StartNew();
        double last = 0.0;

        while (_running)
        {
            double now = sw.Elapsed.TotalSeconds;
            float dt = (float)Math.Min(now - last, 0.1);  // clamp spike dt
            last = now;

            // 1. Commit input for this frame
            lock (_inputLock)
            {
                _heldCommitted.Clear();
                _heldCommitted.UnionWith(_heldPending);
            }

            // 2. Read intent from committed input
            float ax = 0f, ay = 0f;
            if (_heldCommitted.Contains(Keys.A) || _heldCommitted.Contains(Keys.Left)) ax -= THRUST;
            if (_heldCommitted.Contains(Keys.D) || _heldCommitted.Contains(Keys.Right)) ax += THRUST;
            if (_heldCommitted.Contains(Keys.W) || _heldCommitted.Contains(Keys.Up)) ay -= THRUST;
            if (_heldCommitted.Contains(Keys.S) || _heldCommitted.Contains(Keys.Down)) ay += THRUST;

            // 3. Physics — symplectic Euler (velocity first, then position)
            _vx += ax * dt;
            _vy += ay * dt;

            // Drag: exponential decay — velocity *= e^(-drag * dt)
            // e^(-drag*dt) works for any positive drag value.
            // At DRAG=3.0: ~5% velocity remains after 1 second of no thrust.
            float retain = (float)Math.Exp(-DRAG * dt);
            _vx *= retain;
            _vy *= retain;

            _x += _vx * dt;
            _y += _vy * dt;

            // Bounce off screen edges
            if (_x - RADIUS < 0) { _x = RADIUS; _vx = Math.Abs(_vx); }
            if (_x + RADIUS > W) { _x = W - RADIUS; _vx = -Math.Abs(_vx); }
            if (_y - RADIUS < 0) { _y = RADIUS; _vy = Math.Abs(_vy); }
            if (_y + RADIUS > H) { _y = H - RADIUS; _vy = -Math.Abs(_vy); }

            // 4. Render to back buffer
            DrawFrame(dt);

            // 5. Swap buffers and request repaint
            var old = Interlocked.Exchange(ref _front, _back);
            _back = old;

            if (!IsDisposed) Invalidate();

            // 6. Sleep remaining frame budget (target 60 fps)
            double elapsed = sw.Elapsed.TotalSeconds - now;
            int sleep = (int)((1.0 / 60.0 - elapsed) * 1000);
            if (sleep > 1) Thread.Sleep(sleep);
        }
    }

    // -------------------------------------------------------------------------
    // Rendering — draws into _back
    // -------------------------------------------------------------------------
    private void DrawFrame(float dt)
    {
        using (Graphics g = Graphics.FromImage(_back))
        {
            g.SmoothingMode = SmoothingMode.AntiAlias;
            g.Clear(Color.FromArgb(15, 15, 25));  // dark navy background

            DrawGrid(g);
            DrawCircle(g);
            DrawHUD(g);
        }
    }

    private void DrawGrid(Graphics g)
    {
        // Faint grid to make movement visible
        using (var pen = new Pen(Color.FromArgb(30, 255, 255, 255)))
        {
            for (int x = 0; x < W; x += 80)
                g.DrawLine(pen, x, 0, x, H);
            for (int y = 0; y < H; y += 80)
                g.DrawLine(pen, 0, y, W, y);
        }
    }

    private void DrawCircle(Graphics g)
    {
        float speed = (float)Math.Sqrt(_vx * _vx + _vy * _vy);

        // Glow: faint outer circle whose opacity scales with speed
        int glowAlpha = (int)Math.Min(speed / 3f, 120);
        if (glowAlpha > 0)
        {
            using (var glow = new SolidBrush(Color.FromArgb(glowAlpha, 100, 180, 255)))
            {
                float gr = RADIUS + 10f;
                g.FillEllipse(glow, _x - gr, _y - gr, gr * 2, gr * 2);
            }
        }

        // Main circle
        using (var fill = new SolidBrush(Color.CornflowerBlue))
            g.FillEllipse(fill, _x - RADIUS, _y - RADIUS, RADIUS * 2, RADIUS * 2);

        // Outline
        using (var outline = new Pen(Color.White, 2f))
            g.DrawEllipse(outline, _x - RADIUS, _y - RADIUS, RADIUS * 2, RADIUS * 2);

        // Velocity arrow (shows direction and magnitude)
        if (speed > 5f)
        {
            float arrowLen = Math.Min(speed * 0.15f, 60f);
            float nx = _vx / speed;
            float ny = _vy / speed;

            using (var arrow = new Pen(Color.FromArgb(200, 255, 255, 255), 2f))
            {
                arrow.EndCap = LineCap.ArrowAnchor;
                g.DrawLine(arrow,
                    _x, _y,
                    _x + nx * arrowLen,
                    _y + ny * arrowLen);
            }
        }
    }

    private static readonly Font _hudFont = new Font("Consolas", 11f);
    private static readonly Font _hintFont = new Font("Consolas", 10f);

    private void DrawHUD(Graphics g)
    {
        float speed = (float)Math.Sqrt(_vx * _vx + _vy * _vy);

        // Position and speed readout
        string info = string.Format("pos ({0:F0}, {1:F0})   speed {2:F0} px/s",
            _x, _y, speed);
        g.DrawString(info, _hudFont, Brushes.White, 12f, 12f);

        // Key hints at bottom
        DrawKeyHint(g, "W", 32, H - 72, _heldCommitted.Contains(Keys.W) || _heldCommitted.Contains(Keys.Up));
        DrawKeyHint(g, "A", 12, H - 48, _heldCommitted.Contains(Keys.A) || _heldCommitted.Contains(Keys.Left));
        DrawKeyHint(g, "S", 32, H - 48, _heldCommitted.Contains(Keys.S) || _heldCommitted.Contains(Keys.Down));
        DrawKeyHint(g, "D", 52, H - 48, _heldCommitted.Contains(Keys.D) || _heldCommitted.Contains(Keys.Right));
    }

    private void DrawKeyHint(Graphics g, string label, int x, int y, bool active)
    {
        var bg = active
            ? Color.FromArgb(200, 100, 160, 255)
            : Color.FromArgb(80, 255, 255, 255);

        using (var fill = new SolidBrush(bg))
            g.FillRectangle(fill, x, y, 22, 22);

        using (var border = new Pen(Color.White, 1f))
            g.DrawRectangle(border, x, y, 22, 22);

        g.DrawString(label, _hintFont,
            active ? Brushes.White : Brushes.LightGray,
            x + 4f, y + 4f);
    }

    // -------------------------------------------------------------------------
    // WinForms paint — UI thread
    // -------------------------------------------------------------------------
    protected override void OnPaint(PaintEventArgs e)
    {
        var bmp = System.Threading.Volatile.Read(ref _front);
        if (bmp != null) e.Graphics.DrawImage(bmp, 0, 0);
    }

    protected override void OnPaintBackground(PaintEventArgs e) { }

    // -------------------------------------------------------------------------
    // Entry point
    // -------------------------------------------------------------------------
    [STAThread]
    static void Main()
    {
        Application.EnableVisualStyles();
        Application.SetCompatibleTextRenderingDefault(false);

        var form = new CircleDemo();
        form.Show();
        form.Activate();   // explicitly request keyboard focus on Linux/Wayland
        form.StartLoop();
        Application.Run(form);
    }
}
