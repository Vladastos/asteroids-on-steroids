using System.Numerics;

namespace AsteroidsEngine.Engine.Components;

/// <summary>Position and orientation in world space.</summary>
public struct Transform
{
    public Vector2 Position;
    public float   Rotation;  // radians; 0 = pointing right; increases clockwise (GDI+ convention)
}
