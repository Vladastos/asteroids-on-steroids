using System.Drawing;
using System.Drawing.Drawing2D;
using System.Numerics;

namespace AsteroidsEngine.Engine.Rendering;

/// <summary>
/// Defines the viewport into the game world.
/// RenderSystem calls ApplyTo(g) before drawing world-space entities,
/// and ResetTransform(g) before drawing screen-space HUD elements.
///
/// World → screen transform:
///   1. Translate so camera position maps to screen centre
///   2. Scale by Zoom
///
/// Screen → world (for mouse picking):
///   world = (screen - screenCentre) / Zoom + Position
/// </summary>
public sealed class Camera
{
    public Vector2 Position  { get; set; } = Vector2.Zero;
    public float   Zoom      { get; set; } = 1f;

    public int ScreenWidth  { get; set; }
    public int ScreenHeight { get; set; }

    public Camera(int screenWidth, int screenHeight)
    {
        ScreenWidth  = screenWidth;
        ScreenHeight = screenHeight;
    }

    /// <summary>
    /// Applies the camera transform to g. Call before drawing world entities.
    /// Call ResetTransform when done to return to screen space.
    /// </summary>
    public void ApplyTo(Graphics g)
    {
        // Build the matrix manually so scaling is centred on the screen.
        //   1. Translate world origin to screen centre
        //   2. Scale around screen centre
        var m = new Matrix();
        m.Translate(ScreenWidth / 2f - Position.X * Zoom,
                    ScreenHeight / 2f - Position.Y * Zoom);
        m.Scale(Zoom, Zoom, MatrixOrder.Append);
        g.Transform = m;
    }

    public void ResetTransform(Graphics g) => g.ResetTransform();

    /// <summary>Converts a screen pixel position to world coordinates.</summary>
    public Vector2 ScreenToWorld(Point screen) =>
        new((screen.X - ScreenWidth  / 2f) / Zoom + Position.X,
            (screen.Y - ScreenHeight / 2f) / Zoom + Position.Y);

    /// <summary>Converts a world position to screen pixel coordinates.</summary>
    public Point WorldToScreen(Vector2 world) =>
        new((int)((world.X - Position.X) * Zoom + ScreenWidth  / 2f),
            (int)((world.Y - Position.Y) * Zoom + ScreenHeight / 2f));

    /// <summary>
    /// Temporary screen-shake offset. Add to Position before rendering,
    /// remove after. Kept separate so the logical camera position is unchanged.
    /// </summary>
    public void Shake(float magnitude, float durationSeconds)
    {
        // Stored for GameLoop / coroutine to apply each frame.
        // Implemented in Phase 6 (Polish).
        _ = magnitude; _ = durationSeconds;
    }
}
