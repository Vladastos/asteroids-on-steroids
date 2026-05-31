using System.Runtime.InteropServices;
using Silk.NET.SDL;
using AsteroidsEngine.Engine.Rendering;
using DrawPoint   = System.Drawing.Point;
using EngineKey   = AsteroidsEngine.Engine.Input.KeyCode;
using EngineMouse = AsteroidsEngine.Engine.Input.MouseButton;

namespace TwoPlayerGame;

/// <summary>
/// SDL2 implementation of IGameWindow.
///
/// Rendering: caller provides a BGRA8888 pixel buffer (e.g. an SKBitmap pinned
/// via GetPixels()). PresentFrame uploads it to a streaming SDL texture and
/// blits to the screen via the hardware renderer.
///
/// Pixel format compatibility:
///   SkiaSharp SKBitmap (N32 / Bgra8888 on Linux x64) stores B,G,R,A per pixel.
///   SDL ARGB8888 on little-endian also stores B,G,R,A per pixel. → exact match.
/// </summary>
public sealed unsafe class SdlGameWindow : IGameWindow
{
    private readonly Sdl      _sdl;
    private Window*   _window;
    private Renderer* _renderer;
    private Texture*  _texture;
    private bool      _shouldClose;
    private bool      _disposed;

    public int  Width       { get; }
    public int  Height      { get; }
    public bool ShouldClose => _shouldClose;

    public event Action<EngineKey>?              KeyDown;
    public event Action<EngineKey>?              KeyUp;
    public event Action<DrawPoint>?              MouseMoved;
    public event Action<EngineMouse, bool>?      MouseButtonChanged;

    public SdlGameWindow(string title, int width, int height)
    {
        Width  = width;
        Height = height;
        _sdl   = Sdl.GetApi();

        if (_sdl.Init(Sdl.InitVideo | Sdl.InitEvents) < 0)
            Throw("SDL_Init");

        _window = _sdl.CreateWindow(title,
            Sdl.WindowposCentered, Sdl.WindowposCentered,
            width, height, (uint)WindowFlags.Shown);
        if (_window == null) Throw("SDL_CreateWindow");

        _renderer = _sdl.CreateRenderer(_window, -1, (uint)RendererFlags.Accelerated);
        if (_renderer == null) Throw("SDL_CreateRenderer");

        // ARGB8888 in SDL = BGRA in memory on little-endian — matches SkiaSharp N32/Bgra8888.
        _texture = _sdl.CreateTexture(_renderer,
            Sdl.PixelformatArgb8888,
            (int)TextureAccess.Streaming,
            width, height);
        if (_texture == null) Throw("SDL_CreateTexture");
    }

    public void PollEvents()
    {
        Event ev = default;
        while (_sdl.PollEvent(ref ev) != 0)
        {
            switch ((EventType)ev.Type)
            {
                case EventType.Quit:
                    _shouldClose = true;
                    break;
                case EventType.Keydown:
                    if (ev.Key.Repeat == 0)
                    {
                        var key = MapKey((int)ev.Key.Keysym.Sym);
                        if (key.HasValue) KeyDown?.Invoke(key.Value);
                    }
                    break;
                case EventType.Keyup:
                    var upKey = MapKey((int)ev.Key.Keysym.Sym);
                    if (upKey.HasValue) KeyUp?.Invoke(upKey.Value);
                    break;
                case EventType.Mousemotion:
                    MouseMoved?.Invoke(new DrawPoint(ev.Motion.X, ev.Motion.Y));
                    break;
                case EventType.Mousebuttondown:
                    if (ev.Button.Button == 1) MouseButtonChanged?.Invoke(EngineMouse.Left,  true);
                    if (ev.Button.Button == 3) MouseButtonChanged?.Invoke(EngineMouse.Right, true);
                    break;
                case EventType.Mousebuttonup:
                    if (ev.Button.Button == 1) MouseButtonChanged?.Invoke(EngineMouse.Left,  false);
                    if (ev.Button.Button == 3) MouseButtonChanged?.Invoke(EngineMouse.Right, false);
                    break;
            }
        }
    }

    public void PresentFrame(IntPtr pixels, int stride)
    {
        _sdl.UpdateTexture(_texture, null, (void*)pixels, stride);
        _sdl.RenderClear(_renderer);
        _sdl.RenderCopy(_renderer, _texture, null, null);
        _sdl.RenderPresent(_renderer);
    }

    // SDL keycodes: letters use lowercase ASCII (97-122).
    // Engine KeyCode: letters use uppercase ASCII (65-90), matching WinForms Keys.
    // Arrow keycodes derive from (1 << 30) | scancode.
    private static EngineKey? MapKey(int sym) => sym switch
    {
        >= 97  and <= 122 => (EngineKey)(sym - 32), // 'a'-'z' → A-Z
        >= 48  and <= 57  => (EngineKey)sym,          // '0'-'9'
        1073741906        => EngineKey.Up,
        1073741905        => EngineKey.Down,
        1073741904        => EngineKey.Left,
        1073741903        => EngineKey.Right,
        32                => EngineKey.Space,
        27                => EngineKey.Escape,
        13                => EngineKey.Enter,
        9                 => EngineKey.Tab,
        8                 => EngineKey.Back,
        _                 => null
    };

    private void Throw(string call)
    {
        string err = Marshal.PtrToStringAnsi((IntPtr)_sdl.GetError()) ?? "unknown error";
        throw new InvalidOperationException($"{call} failed: {err}");
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;

        if (_texture  != null) { _sdl.DestroyTexture(_texture);   _texture  = null; }
        if (_renderer != null) { _sdl.DestroyRenderer(_renderer); _renderer = null; }
        if (_window   != null) { _sdl.DestroyWindow(_window);     _window   = null; }

        _sdl.Quit();
        _sdl.Dispose();
    }
}
