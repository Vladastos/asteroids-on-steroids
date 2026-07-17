using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Numerics;
using AsteroidsEngine.Engine.Rendering;
using EngineColor = AsteroidsEngine.Engine.Rendering.Color;   // disambiguate from System.Drawing.Color
using GdiColor    = System.Drawing.Color;
using GdiMatrix   = System.Drawing.Drawing2D.Matrix;

namespace AsteroidsEngine.Platform.WinForms;

/// <summary>
/// GDI+ implementation of the engine's <see cref="IRenderer"/> (and, approximately,
/// <see cref="IPostEffects"/>). Draws into the window's back-buffer <see cref="Bitmap"/>; the
/// <see cref="WinFormsGameWindow"/> blits that buffer to the screen in Present(). Mirrors the
/// structure of the SkiaSharp backend — one reusable brush/pen, a font cache, a transform stack.
/// </summary>
public sealed class GdiRenderer : IRenderer, IPostEffects, IDisposable
{
    private readonly Bitmap   _backBuffer;
    private readonly Graphics _g;
    private readonly int      _width, _height;

    private readonly SolidBrush _brush = new(GdiColor.White);
    private readonly Pen        _pen   = new(GdiColor.White);
    private readonly Dictionary<string, Font> _fonts = new();
    private readonly Stack<GraphicsState>     _stack = new();
    private readonly GraphicsPath             _path  = new() { FillMode = FillMode.Winding };
    private readonly StringFormat _fmt = new(StringFormat.GenericTypographic)
        { FormatFlags = StringFormatFlags.MeasureTrailingSpaces };

    private PointF[] _poly = Array.Empty<PointF>();          // reused vertex scratch
    private Bitmap?  _warpSrc;                               // reused Distort snapshot

    public GdiRenderer(Bitmap backBuffer, Graphics graphics, int width, int height)
    {
        _backBuffer = backBuffer;
        _g          = graphics;
        _width      = width;
        _height     = height;
        _g.SmoothingMode     = SmoothingMode.AntiAlias;
        _g.TextRenderingHint = System.Drawing.Text.TextRenderingHint.AntiAlias;
        _g.PixelOffsetMode   = PixelOffsetMode.HighQuality;
        _g.InterpolationMode = InterpolationMode.Bilinear;
    }

    public void Begin(EngineColor clear)
    {
        _g.ResetTransform();
        _g.Clear(ToGdi(clear));
    }

    public void End() { /* GDI+ draws straight to the back-buffer Bitmap; nothing to flush. */ }

    public void PushTransform(in Matrix3x2 transform)
    {
        _stack.Push(_g.Save());
        // Prepend to match Skia's Save()+Concat(): the pushed matrix applies first to points.
        _g.MultiplyTransform(ToGdi(transform), MatrixOrder.Prepend);
    }

    public void PopTransform()
    {
        if (_stack.Count > 0) _g.Restore(_stack.Pop());
    }

    public void DrawLine(Vector2 a, Vector2 b, EngineColor color, float width = 1f)
    {
        _pen.Color = ToGdi(color); _pen.Width = width;
        _g.DrawLine(_pen, a.X, a.Y, b.X, b.Y);
    }

    public void DrawPolygon(ReadOnlySpan<Vector2> verts, EngineColor color, float width = 1f)
    {
        if (verts.Length < 2) return;
        _pen.Color = ToGdi(color); _pen.Width = width;
        _g.DrawPolygon(_pen, ToPoly(verts));
    }

    public void FillPolygon(ReadOnlySpan<Vector2> verts, EngineColor color)
    {
        if (verts.Length < 3) return;
        _brush.Color = ToGdi(color);
        _g.FillPolygon(_brush, ToPoly(verts));
    }

    public void FillPath(ReadOnlySpan<Vector2> verts, ReadOnlySpan<int> contourLengths, EngineColor color)
    {
        _path.Reset();
        _path.FillMode = FillMode.Winding;   // nonzero union → seamless, matches Skia's FillPath
        int off = 0;
        foreach (int len in contourLengths)
        {
            if (len >= 3)
            {
                var pts = new PointF[len];
                for (int i = 0; i < len; i++) pts[i] = new PointF(verts[off + i].X, verts[off + i].Y);
                _path.AddPolygon(pts);
            }
            off += len;
        }
        if (_path.PointCount > 0)
        {
            _brush.Color = ToGdi(color);
            _g.FillPath(_brush, _path);
        }
    }

    public void DrawCircle(Vector2 center, float radius, EngineColor color, float width = 1f)
    {
        _pen.Color = ToGdi(color); _pen.Width = width;
        _g.DrawEllipse(_pen, center.X - radius, center.Y - radius, radius * 2f, radius * 2f);
    }

    public void FillCircle(Vector2 center, float radius, EngineColor color)
    {
        _brush.Color = ToGdi(color);
        _g.FillEllipse(_brush, center.X - radius, center.Y - radius, radius * 2f, radius * 2f);
    }

    public void DrawText(string text, Vector2 topLeft, EngineColor color, in FontSpec font)
    {
        _brush.Color = ToGdi(color);
        _g.DrawString(text, GetFont(font), _brush, topLeft.X, topLeft.Y, _fmt);
    }

    public Vector2 MeasureText(string text, in FontSpec font)
    {
        SizeF s = _g.MeasureString(text, GetFont(font), int.MaxValue, _fmt);
        return new Vector2(s.Width, s.Height);
    }

    // ── IPostEffects: approximate the mesh warp with per-cell affine DrawImage ──────────────────
    public void Distort(Vector2 regionMin, Vector2 regionMax, int gridX, int gridY, Func<Vector2, Vector2> sourceOf)
    {
        int x0 = (int)MathF.Floor(Math.Clamp(MathF.Min(regionMin.X, regionMax.X), 0, _width));
        int y0 = (int)MathF.Floor(Math.Clamp(MathF.Min(regionMin.Y, regionMax.Y), 0, _height));
        int x1 = (int)MathF.Ceiling(Math.Clamp(MathF.Max(regionMin.X, regionMax.X), 0, _width));
        int y1 = (int)MathF.Ceiling(Math.Clamp(MathF.Max(regionMin.Y, regionMax.Y), 0, _height));
        int rw = x1 - x0, rh = y1 - y0;
        if (rw < 2 || rh < 2) return;
        gridX = Math.Max(1, gridX); gridY = Math.Max(1, gridY);

        // The back-buffer already holds everything drawn so far (GDI+ is immediate), so cloning the
        // region is a free "snapshot". _warpSrc pixel (0,0) maps to region origin (x0,y0).
        _warpSrc?.Dispose();
        _warpSrc = _backBuffer.Clone(new Rectangle(x0, y0, rw, rh), _backBuffer.PixelFormat);
        var origin = new Vector2(x0, y0);

        using var attrs = new ImageAttributes();
        attrs.SetWrapMode(WrapMode.Clamp);   // don't tile when a source rect spills past the snapshot

        var saved = _g.Save();
        _g.ResetTransform();                 // Distort is screen-space
        _g.SetClip(new RectangleF(x0, y0, rw, rh));

        float cw = rw / (float)gridX, ch = rh / (float)gridY;
        var dest = new PointF[3];
        for (int j = 0; j < gridY; j++)
        for (int i = 0; i < gridX; i++)
        {
            float dx0 = x0 + i * cw, dy0 = y0 + j * ch, dx1 = dx0 + cw, dy1 = dy0 + ch;
            // Source (snapshot-local) positions the four dest corners sample from.
            Vector2 sA = sourceOf(new Vector2(dx0, dy0)) - origin;
            Vector2 sB = sourceOf(new Vector2(dx1, dy0)) - origin;
            Vector2 sC = sourceOf(new Vector2(dx0, dy1)) - origin;
            Vector2 sD = sourceOf(new Vector2(dx1, dy1)) - origin;
            float smnx = MathF.Min(MathF.Min(sA.X, sB.X), MathF.Min(sC.X, sD.X));
            float smny = MathF.Min(MathF.Min(sA.Y, sB.Y), MathF.Min(sC.Y, sD.Y));
            float smxx = MathF.Max(MathF.Max(sA.X, sB.X), MathF.Max(sC.X, sD.X));
            float smxy = MathF.Max(MathF.Max(sA.Y, sB.Y), MathF.Max(sC.Y, sD.Y));
            var srcRect = new RectangleF(smnx, smny, MathF.Max(1f, smxx - smnx), MathF.Max(1f, smxy - smny));

            // dest parallelogram: upper-left, upper-right, lower-left (4th corner inferred).
            dest[0] = new PointF(dx0, dy0);
            dest[1] = new PointF(dx1, dy0);
            dest[2] = new PointF(dx0, dy1);
            _g.DrawImage(_warpSrc, dest, srcRect, GraphicsUnit.Pixel, attrs);
        }

        _g.ResetClip();
        _g.Restore(saved);
    }

    // ── Helpers ─────────────────────────────────────────────────────────────────
    private PointF[] ToPoly(ReadOnlySpan<Vector2> verts)
    {
        if (_poly.Length != verts.Length) _poly = new PointF[verts.Length];
        for (int i = 0; i < verts.Length; i++) _poly[i] = new PointF(verts[i].X, verts[i].Y);
        return _poly;
    }

    private Font GetFont(in FontSpec f)
    {
        string key = $"{f.Family}|{f.Size}|{f.Bold}";
        if (!_fonts.TryGetValue(key, out var font))
        {
            // Pixel unit so Size matches Skia's TextSize (which is in px, not points).
            font = new Font(f.Family, f.Size, f.Bold ? FontStyle.Bold : FontStyle.Regular, GraphicsUnit.Pixel);
            _fonts[key] = font;
        }
        return font;
    }

    private static GdiColor ToGdi(EngineColor c) => GdiColor.FromArgb(c.A, c.R, c.G, c.B);

    // System.Numerics.Matrix3x2 and System.Drawing Matrix share the row-vector layout.
    private static GdiMatrix ToGdi(in Matrix3x2 m) => new(m.M11, m.M12, m.M21, m.M22, m.M31, m.M32);

    public void Dispose()
    {
        _brush.Dispose();
        _pen.Dispose();
        _path.Dispose();
        _fmt.Dispose();
        _warpSrc?.Dispose();
        foreach (var f in _fonts.Values) f.Dispose();
        _fonts.Clear();
    }
}
