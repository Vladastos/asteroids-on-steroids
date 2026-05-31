using System.Text.Json;
using System.Text.Json.Serialization;
using AsteroidsEngine.Engine.Components;

namespace AsteroidDemo;

// ---------------------------------------------------------------------------
// Top-level config
// ---------------------------------------------------------------------------

public class GameConfig
{
    public WindowConfig Window { get; set; } = new();
    public PlayerConfig Player { get; set; } = new();
    public BulletConfig Bullet { get; set; } = new();
    public AsteroidConfig Asteroid { get; set; } = new();
    public WaveConfig Waves { get; set; } = new();
    public FractureConfig Fracture { get; set; } = new();

    // ---- Loading ----

    public static GameConfig Load(string path = "config.json")
    {
        if (!File.Exists(path))
            path = Path.Combine(AppContext.BaseDirectory, "config.json");

        if (!File.Exists(path))
        {
            Console.WriteLine("[config] config.json not found — using built-in defaults.");
            return new GameConfig();
        }

        try
        {
            string json = File.ReadAllText(path);
            var opts = new JsonSerializerOptions
            {
                PropertyNameCaseInsensitive = true,
                ReadCommentHandling = JsonCommentHandling.Skip,
                AllowTrailingCommas = true,
            };
            var cfg = JsonSerializer.Deserialize<GameConfig>(json, opts);
            Console.WriteLine($"[config] Loaded {path}");
            return cfg ?? new GameConfig();
        }
        catch (Exception ex)
        {
            Console.WriteLine($"[config] Failed to parse config.json: {ex.Message} — using defaults.");
            return new GameConfig();
        }
    }

    // ---- Helpers ----

    /// <summary>Builds a FractureProperties from the named material preset in the config.</summary>
    public FractureProperties GetMaterial(string name)
    {
        if (Fracture.Materials.TryGetValue(name, out var m))
            return new FractureProperties
            {
                Brittleness = m.Brittleness,
                Toughness = m.Toughness,
                FaultCount = m.FaultCount,
                MinFragmentArea = m.MinFragmentArea,
            };

        Console.WriteLine($"[config] Unknown material '{name}' — falling back to Rock preset.");
        return FractureProperties.Rock;
    }

    /// <summary>Returns the material name to use for the given 1-based wave number.</summary>
    public string MaterialForWave(int wave)
    {
        var list = Fracture.WaveMaterials;
        if (list.Count == 0) return "rock";
        int idx = Math.Clamp(wave - 1, 0, list.Count - 1);
        return list[idx];
    }
}

// ---------------------------------------------------------------------------
// Sections
// ---------------------------------------------------------------------------

public class WindowConfig
{
    public int Width { get; set; } = 1280;
    public int Height { get; set; } = 720;
    public string Title { get; set; } = "Asteroid Demo - F to shoot";
}

public class PlayerConfig
{
    public float Radius { get; set; } = 18f;
    public float Mass { get; set; } = 2f;
    public float LinearDrag { get; set; } = 3.0f;
    public float Thrust { get; set; } = 1600f;
    public float Restitution { get; set; } = 0.3f;
}

public class BulletConfig
{
    public float Radius { get; set; } = 5f;
    public float Speed { get; set; } = 1820f;
    public float Mass { get; set; } = 0.1f;
    public float Ttl { get; set; } = 2.2f;
    public float Cooldown { get; set; } = 0.1f;
}

public class AsteroidConfig
{
    public float RadiusMin { get; set; } = 38f;
    public float RadiusMax { get; set; } = 140f;
    public float Density { get; set; } = 0.004f;   // mass = radius² × π × density
    public float SpeedMin { get; set; } = 30f;
    public float SpeedRange { get; set; } = 100f;     // speed = SpeedMin + rng × SpeedRange
    public float SpinMax { get; set; } = 1.2f;     // max |angular velocity| in rad/s
    public float LinearDrag { get; set; } = 0.01f;
    public float AngularDrag { get; set; } = 0.04f;
    public float Restitution { get; set; } = 0.4f;
}

public class WaveConfig
{
    public int InitialCount { get; set; } = 6;
    public int CountPerWave { get; set; } = 2;    // added each wave
    public float NextWaveDelaySecs { get; set; } = 2.0f;
    public float SafeCentreRadius { get; set; } = 200f; // spawn exclusion zone around player
}

public class FractureConfig
{
    // Energy model constants
    // Note: bullet mass comes from BulletConfig.Mass — no duplication here.
    public float SpinEnergyFraction { get; set; } = 0.35f;  // fraction of I×ω² added to E_total
    public float MomentumTransfer { get; set; } = 0.55f;  // bullet momentum → fragment kick
    public float MassReference { get; set; } = 4f;     // m_ref for spread-speed normalisation
    public float ImpactFlashTtl { get; set; } = 0.45f;

    // Zone geometry — physics-based (see physics_spec.md §5)
    // fractureZoneRadius = sqrt(E_fracture / (Toughness × density) / π)
    // clamped to [MinFractureZoneFraction × rAsteroid, rAsteroid]
    public float MinFractureZoneFraction { get; set; } = 0.05f;  // minimum zone as fraction of rAsteroid
    public float BlastFractionDuctile { get; set; } = 0.40f;  // blast/zone ratio for ductile materials
    public float BlastFractionBrittle { get; set; } = 0.10f;  // blast/zone ratio for brittle materials
    public float BlastMin { get; set; } = 2f;      // absolute minimum blast radius (px)
    public float BlastMax { get; set; } = 60f;     // absolute maximum blast radius (px)

    // Inner cut angular spread
    public float SpreadAngleMin { get; set; } = 0.3f;  // lerp(min, max, Brittleness) × π
    public float SpreadAngleMax { get; set; } = 1.0f;

    // Secondary (radial) cut count — K-1 cuts forming the "bite"
    public int SecondaryCutsMin { get; set; } = 0;   // K-1 at Brittleness=0
    public int SecondaryCutsMax { get; set; } = 5;   // K-1 at Brittleness=1
    public int MaxSecondaryCuts { get; set; } = 7;
    public float ConeHalfAngleDeg { get; set; } = 90f; // half-angle of the secondary cut fan

    // Inner (impact-zone) cut count
    public int InnerCutsMin { get; set; } = 1;
    public int InnerCutsMax { get; set; } = 7;
    public int MaxInnerCuts { get; set; } = 10;

    // Wave → material progression (index 0 = wave 1; last entry repeats)
    public List<string> WaveMaterials { get; set; } =
        ["rock", "rock", "ice", "ice", "glass", "metal"];

    // Material presets (keyed by name; used in WaveMaterials)
    public Dictionary<string, MaterialConfig> Materials { get; set; } = new(StringComparer.OrdinalIgnoreCase)
    {
        ["glass"] = new() { Brittleness = 1.00f, Toughness = 840f, FaultCount = 0, MinFragmentArea = 40f },
        ["ice"] = new() { Brittleness = 0.80f, Toughness = 2_100f, FaultCount = 7, MinFragmentArea = 80f },
        ["rock"] = new() { Brittleness = 0.60f, Toughness = 8_400f, FaultCount = 4, MinFragmentArea = 180f },
        ["metal"] = new() { Brittleness = 0.15f, Toughness = 84_000f, FaultCount = 2, MinFragmentArea = 400f },
    };

    // Fragment physics
    public FragmentPhysicsConfig Fragment { get; set; } = new();

    // Debris (sub-threshold small fragments)
    public DebrisConfig Debris { get; set; } = new();

    // Sub-threshold hit debris cloud (surface sparks)
    public DebrisCloudConfig DebrisCloud { get; set; } = new();
}

public class MaterialConfig
{
    public float Brittleness { get; set; }
    public float Toughness { get; set; }
    public int FaultCount { get; set; }
    public float MinFragmentArea { get; set; }
}

public class FragmentPhysicsConfig
{
    public float FarPieceBaseSpeed { get; set; } = 20f;
    public float NearBaseSpeedMin { get; set; } = 55f;
    public float NearBaseSpeedRange { get; set; } = 90f;   // speed = min + rng × range
    public float SpreadNormMin { get; set; } = 0.3f;  // (SpreadNormMin + sevNorm × (1-min)) scaling
    public float LinearDrag { get; set; } = 0.02f;
    public float AngularDrag { get; set; } = 0.08f;
    public float Restitution { get; set; } = 0.6f;  // player↔asteroid elastic bounce
}

public class DebrisConfig
{
    public float NormalSpeedMin { get; set; } = 40f;
    public float NormalSpeedRange { get; set; } = 80f;
    public float BlastSpeedMin { get; set; } = 180f;
    public float BlastSpeedRange { get; set; } = 220f;
    public float NormalTtlMin { get; set; } = 0.4f;
    public float NormalTtlMax { get; set; } = 1.0f;
    public float BlastTtlMin { get; set; } = 0.15f;
    public float BlastTtlMax { get; set; } = 0.35f;
    public float NormalLinearDrag { get; set; } = 1.5f;
    public float BlastLinearDrag { get; set; } = 2.5f;
    public float Mass { get; set; } = 0.1f;
}

public class DebrisCloudConfig
{
    public int Count { get; set; } = 4;
    public float SpeedMin { get; set; } = 60f;
    public float SpeedMax { get; set; } = 140f;
    public float LateralSpread { get; set; } = 100f;
    public float SizeMin { get; set; } = 4f;
    public float SizeMax { get; set; } = 10f;
    public float TtlMin { get; set; } = 0.25f;
    public float TtlMax { get; set; } = 0.60f;
    public float Mass { get; set; } = 0.05f;
    public float LinearDrag { get; set; } = 2.0f;
}
