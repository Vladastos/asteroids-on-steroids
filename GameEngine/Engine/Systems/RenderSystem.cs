using System.Drawing;
// Legacy GDI+ render path (unused by the SDL/Skia demos). 'Color' here means the
// GDI+ colour; the new backend-agnostic AsteroidsEngine.Engine.Rendering.Color is
// used by the IRenderer path. This whole file is slated for removal once the
// sprite path is migrated to IRenderer.
using Color = System.Drawing.Color;
using AsteroidsEngine.Engine.Components;
using AsteroidsEngine.Engine.Core;
using AsteroidsEngine.Engine.Rendering;
using AsteroidsEngine.Engine.Resources;

namespace AsteroidsEngine.Engine.Systems;

/// <summary>
/// Draws all entities that have both Transform and Sprite components.
///
/// Draw order: sorted ascending by Sprite.Layer (lower = drawn first = behind).
/// DisabledTag entities are skipped.
///
/// Called by GameWindow.OnGameDraw, which is invoked from the game thread
/// after all ISystem.Update calls complete.
/// </summary>
public sealed class RenderSystem : IDrawSystem
{
    private readonly Camera          _camera;
    private readonly ResourceManager _resources;

    // Reused buffer to avoid allocating a new list each frame.
    private readonly List<(float x, float y, float rot, Sprite sprite)> _drawList = new();

    public RenderSystem(Camera camera, ResourceManager resources)
    {
        _camera    = camera;
        _resources = resources;
    }

    public void Draw(World world, Graphics g)
    {
        // --- Collect visible entities ---
        _drawList.Clear();

        world.ForEach<Transform, Sprite>((Entity e, ref Transform t, ref Sprite s) =>
        {
            if (world.HasComponent<DisabledTag>(e)) return;
            if (string.IsNullOrEmpty(s.ImageId))   return;
            _drawList.Add((t.Position.X + s.Offset.X,
                           t.Position.Y + s.Offset.Y,
                           t.Rotation,
                           s));
        });

        // Sort by layer (stable-ish; List.Sort is not stable but layer ties
        // are visually unordered anyway so that's acceptable).
        _drawList.Sort((a, b) => a.sprite.Layer.CompareTo(b.sprite.Layer));

        // --- Apply camera transform and draw ---
        var worldState = g.Save();
        _camera.ApplyTo(g);

        foreach (var (x, y, rot, sprite) in _drawList)
        {
            var image = _resources.GetImage(sprite.ImageId);
            if (image == null) continue;

            DrawSprite(g, image, x, y, rot, sprite.Tint);
        }

        g.Restore(worldState);
    }

    private static void DrawSprite(Graphics g, Bitmap image,
                                   float x, float y, float rotation, Color tint)
    {
        // Save/Restore the full graphics state (transform + clip) around each
        // sprite so per-entity transforms don't accumulate.
        var state = g.Save();

        // Rotate around the sprite's centre point.
        g.TranslateTransform(x, y);
        g.RotateTransform(rotation * (180f / MathF.PI));  // GDI+ uses degrees
        g.TranslateTransform(-image.Width / 2f, -image.Height / 2f);

        if (tint == Color.White || tint == Color.Empty)
        {
            g.DrawImage(image, 0, 0);
        }
        else
        {
            // Apply tint via a ColorMatrix without allocating a new Bitmap.
            using var attrs = TintAttributes(tint);
            var dest = new Rectangle(0, 0, image.Width, image.Height);
            g.DrawImage(image, dest, 0, 0, image.Width, image.Height,
                        GraphicsUnit.Pixel, attrs);
        }

        g.Restore(state);
    }

    private static System.Drawing.Imaging.ImageAttributes TintAttributes(Color tint)
    {
        float r = tint.R / 255f;
        float gr = tint.G / 255f;
        float b = tint.B / 255f;
        float a = tint.A / 255f;

        var matrix = new System.Drawing.Imaging.ColorMatrix(new float[][]
        {
            new[] { r,  0,  0,  0, 0 },
            new[] { 0, gr,  0,  0, 0 },
            new[] { 0,  0,  b,  0, 0 },
            new[] { 0,  0,  0,  a, 0 },
            new[] { 0f, 0, 0,   0, 1 },
        });

        var attrs = new System.Drawing.Imaging.ImageAttributes();
        attrs.SetColorMatrix(matrix);
        return attrs;
    }
}
