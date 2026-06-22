namespace AsteroidsGame.Components;

public struct PlayerTag      { }
public struct AsteroidTag    { }
public struct AlienTag       { }
public struct BulletTag      { }
/// <summary>Marker on bullets fired by alien ships so they can target the player layer.</summary>
public struct AlienBulletTag { }

/// <summary>Key into GameConfig.Asteroids (e.g. "standard", "boulder").</summary>
public struct AsteroidVariant { public string Key; }

/// <summary>Key into GameConfig.Entities (e.g. "drone", "bruiser", "mothership").</summary>
public struct AlienVariant { public string Key; }
