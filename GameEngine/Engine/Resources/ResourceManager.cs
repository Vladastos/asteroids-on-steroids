using System.Drawing;

namespace AsteroidsEngine.Engine.Resources;

/// <summary>
/// Loads and caches assets. Full implementation in Phase 4.
/// All methods are safe to call before assets are loaded (return null/default).
/// </summary>
public sealed class ResourceManager : IDisposable
{
    private readonly Dictionary<string, Bitmap> _images = new();
    private readonly Dictionary<string, Font>   _fonts  = new();
    private bool _disposed;

    // -------------------------------------------------------------------------
    // Images
    // -------------------------------------------------------------------------

    public Bitmap? GetImage(string id)
    {
        _images.TryGetValue(id, out var bmp);
        return bmp;
    }

    public void LoadImage(string id, string path)
    {
        if (_images.ContainsKey(id)) return;
        _images[id] = new Bitmap(path);
    }

    public void RegisterImage(string id, Bitmap bitmap)
    {
        _images[id] = bitmap;
    }

    // -------------------------------------------------------------------------
    // Fonts
    // -------------------------------------------------------------------------

    public Font GetFont(string id, float size = 12f)
    {
        string key = $"{id}:{size}";
        if (!_fonts.TryGetValue(key, out var font))
        {
            font = new Font(id, size);
            _fonts[key] = font;
        }
        return font;
    }

    // -------------------------------------------------------------------------
    // Lifecycle
    // -------------------------------------------------------------------------

    public void Clear()
    {
        foreach (var b in _images.Values) b.Dispose();
        foreach (var f in _fonts.Values)  f.Dispose();
        _images.Clear();
        _fonts.Clear();
    }

    public void Dispose()
    {
        if (_disposed) return;
        Clear();
        _disposed = true;
    }
}
