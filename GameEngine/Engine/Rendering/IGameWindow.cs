using AsteroidsEngine.Engine.Input;

namespace AsteroidsEngine.Engine.Rendering;

/// <summary>
/// Platform-agnostic game window contract.
///
/// The game loop drives rendering by:
///   1. Calling PollEvents() each frame — drains OS events → fires KeyDown/KeyUp
///   2. Drawing to any pixel buffer (SkiaSharp SKBitmap, GDI+ Bitmap, etc.)
///   3. Calling PresentFrame(pixels, stride) to upload and display the frame
///
/// PresentFrame takes a raw BGRA8888 pixel pointer (platform-agnostic):
///   SDL2:     uploads to a streaming texture and calls RenderPresent
///   WinForms: wraps in a GDI Bitmap and blits via Graphics.DrawImage
///
/// Current implementations:
///   SdlGameWindow  — Linux/Mac/Windows via SDL2  (TwoPlayerGame project)
///   GameWindow     — Windows via WinForms         (to be added in Windows phase)
/// </summary>
public interface IGameWindow : IDisposable
{
    int  Width       { get; }
    int  Height      { get; }
    bool ShouldClose { get; }

    /// <summary>Fired when a key is pressed (no repeat events).</summary>
    event Action<KeyCode>? KeyDown;

    /// <summary>Fired when a key is released.</summary>
    event Action<KeyCode>? KeyUp;

    /// <summary>
    /// Drain pending OS events and fire KeyDown/KeyUp callbacks.
    /// SDL2:     polls the SDL event queue.
    /// WinForms: no-op — the message pump runs on the main thread.
    /// </summary>
    void PollEvents();

    /// <summary>
    /// Display the rendered frame. <paramref name="pixels"/> must point to a
    /// contiguous BGRA8888 pixel buffer of size Height * <paramref name="stride"/>
    /// bytes. The buffer must remain valid for the duration of this call.
    /// </summary>
    void PresentFrame(IntPtr pixels, int stride);
}
