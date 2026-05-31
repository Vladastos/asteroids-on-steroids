using System.Numerics;

namespace AsteroidsEngine.Engine.Destruction;

/// <summary>
/// One convex Voronoi cell of a fracturable body, expressed in body-local space
/// (centroid-relative, matching PolygonShape). The union of a body's cells is its
/// (possibly concave) silhouette; each cell on its own is always convex.
/// </summary>
public struct Cell
{
    public Vector2[] Local;     // convex polygon, body-local vertices
    public Vector2   Centroid;  // body-local centroid of this cell
    public float     Area;      // |area| of the cell (px²)
}
