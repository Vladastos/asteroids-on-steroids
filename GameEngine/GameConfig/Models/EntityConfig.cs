namespace AsteroidsGame.Config;

/// <summary>Alien ship prefab config. Shape file is loaded from Assets/shapes/.</summary>
public class EntityConfig
{
    public string           Shape                    { get; set; } = "";
    public string           Material                 { get; set; } = "metal";
    public float            Speed                    { get; set; } = 200f;
    public float            DetectionRadius          { get; set; } = 800f;
    public SteeringWeights? SteeringWeights          { get; set; }
    public float            Thrust                   { get; set; } = 600f;
    public float            ShootCooldown            { get; set; } = 2f;
    public float            LateralThrustPenaltyMult { get; set; } = 0.4f;
    public float            AlienImpactCoeff         { get; set; } = 1.0f;
    public float            ShapeScale               { get; set; } = 1.0f;
    public float            BaseCost                 { get; set; } = 20f;
    public int              CellCount                { get; set; } = 8;
}

public class SteeringWeights
{
    public float Separation { get; set; } = 1f;
    public float Pursuit    { get; set; } = 1f;
    public float Avoidance  { get; set; } = 1f;
}
