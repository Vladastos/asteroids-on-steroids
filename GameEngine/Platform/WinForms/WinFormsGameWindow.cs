using System.Drawing;
using System.Numerics;
using System.Windows.Forms;
using AsteroidsEngine.Engine.Input;
using AsteroidsEngine.Engine.Rendering;

namespace AsteroidsEngine.Platform.WinForms;

/// <summary>
/// WinForms + GDI+ implementation of <see cref="IGameWindow"/>. A borderless (optionally fullscreen)
/// <see cref="Form"/> that owns a back-buffer <see cref="Bitmap"/> the <see cref="GdiRenderer"/> draws
/// into; <see cref="Present"/> blits it to the client area. The engine owns the loop, so
/// <see cref="PollEvents"/> pumps the Win32 message queue via <see cref="Application.DoEvents"/> instead
/// of ever calling <see cref="Application.Run"/>.
/// </summary>
public sealed class WinFormsGameWindow : Form, IGameWindow
{
    private readonly int      _width, _height;
    private readonly Bitmap   _backBuffer;
    private readonly Graphics _bufferGraphics;
    private readonly GdiRenderer _renderer;
    private bool _shouldClose;

    public WinFormsGameWindow(string title, int width, int height, bool fullscreen = true)
    {
        _width  = width;
        _height = height;

        Text = title;
        // Paint every pixel ourselves; suppress the flicker of WinForms' own erase/paint.
        SetStyle(ControlStyles.UserPaint | ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer, true);
        FormBorderStyle = FormBorderStyle.None;
        if (fullscreen)
        {
            StartPosition = FormStartPosition.Manual;
            Bounds        = Screen.PrimaryScreen!.Bounds;
            TopMost       = true;
        }
        else
        {
            StartPosition = FormStartPosition.CenterScreen;
            ClientSize    = new Size(width, height);
        }
        KeyPreview = true;   // receive key events even when a child control has focus (there are none, but safe)

        _backBuffer     = new Bitmap(width, height);
        _bufferGraphics = Graphics.FromImage(_backBuffer);
        _renderer       = new GdiRenderer(_backBuffer, _bufferGraphics, width, height);

        // ── Input: raise the PAL events from the WinForms events ──────────────
        // KeyDown/KeyUp collide with Form's own events, so they're implemented explicitly (below)
        // and wired here via the base Control events.
        base.KeyDown += (_, e) => _keyDown?.Invoke((KeyCode)(int)e.KeyCode);
        base.KeyUp   += (_, e) => _keyUp?.Invoke((KeyCode)(int)e.KeyCode);
        MouseMove    += (_, e) => MouseMoved?.Invoke(new Vector2(e.X, e.Y));
        MouseDown    += (_, e) => MouseButtonChanged?.Invoke(WinToEngine(e.Button), true);
        MouseUp      += (_, e) => MouseButtonChanged?.Invoke(WinToEngine(e.Button), false);
        KeyPress     += (_, e) =>
        {
            // Printable characters only, and not while Ctrl/Alt are held (mirrors the SDL backend).
            if (!char.IsControl(e.KeyChar) && (Control.ModifierKeys & (Keys.Control | Keys.Alt)) == 0)
                TextInput?.Invoke(e.KeyChar.ToString());
        };

        Show();
        Application.DoEvents();   // realise the handle so CreateGraphics() works before the first Present
    }

    /// <summary>Primary-monitor resolution, mirroring SdlGameWindow.QueryDisplaySize.</summary>
    public static (int, int) QueryDisplaySize()
    {
        var b = Screen.PrimaryScreen?.Bounds ?? new Rectangle(0, 0, 1920, 1080);
        return (b.Width, b.Height);
    }

    // ── IGameWindow ─────────────────────────────────────────────────────────────
    public new int Width  => _width;    // 'new' hides Form.Width (outer window size); we want the render size
    public new int Height => _height;
    public bool ShouldClose => _shouldClose;
    public IRenderer Renderer => _renderer;

    // KeyDown/KeyUp are explicit — their names collide with Form.KeyDown/KeyUp.
    private Action<KeyCode>? _keyDown, _keyUp;
    event Action<KeyCode>? IGameWindow.KeyDown { add => _keyDown += value; remove => _keyDown -= value; }
    event Action<KeyCode>? IGameWindow.KeyUp   { add => _keyUp   += value; remove => _keyUp   -= value; }

    public event Action<Vector2>?             MouseMoved;
    public event Action<MouseButton, bool>?   MouseButtonChanged;
    public event Action<string>?              TextInput;

    public void PollEvents() => Application.DoEvents();   // pump the queue; the wired handlers fire the events

    public void Present()
    {
        using var g = CreateGraphics();
        g.DrawImageUnscaled(_backBuffer, 0, 0);
    }

    // ── Form overrides ──────────────────────────────────────────────────────────
    protected override void OnFormClosing(FormClosingEventArgs e)
    {
        _shouldClose = true;
        base.OnFormClosing(e);
    }

    protected override void OnPaint(PaintEventArgs e)
        => e.Graphics.DrawImageUnscaled(_backBuffer, 0, 0);   // handle expose events during DoEvents

    protected override void OnPaintBackground(PaintEventArgs e) { /* covered every frame */ }

    protected override void Dispose(bool disposing)
    {
        if (disposing)
        {
            _renderer.Dispose();
            _bufferGraphics.Dispose();
            _backBuffer.Dispose();
        }
        base.Dispose(disposing);
    }

    private static MouseButton WinToEngine(MouseButtons b) => b switch
    {
        MouseButtons.Left   => MouseButton.Left,
        MouseButtons.Right  => MouseButton.Right,
        MouseButtons.Middle => MouseButton.Middle,
        _                   => MouseButton.Left,
    };
}
