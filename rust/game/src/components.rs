//! Core ECS components ported from the C# engine/gameplay component layer.

use bevy::prelude::*;

/// Attaches a collision shape to an entity.
///
/// Port of C# `Engine/Components/Collider.cs`.
#[derive(Component)]
pub struct Collider {
    pub shape: collision::Shape,
    pub layer: i32,
    pub mask: i32,
    pub sensor: bool,
}

/// Previous fixed-step pose captured for render interpolation.
///
/// Port of the previous-pose half of C# `Engine/Components/Transform.cs`.
/// Current pose stays on Bevy's built-in `Transform`.
#[derive(Component, Default)]
pub struct PreviousTransform {
    pub position: Vec2,
    pub rotation: f32,
}

/// Linear and angular velocity consumed by movement/physics systems.
///
/// Port of C# `Engine/Components/Velocity.cs`.
#[derive(Component, Default)]
pub struct Velocity {
    /// Units per second.
    pub linear: Vec2,
    /// Radians per second; clockwise positive to match the source game.
    pub angular: f32,
}

/// Physics properties consumed by force integration and collision response.
///
/// Port of C# `Engine/Components/RigidBody.cs`.
#[derive(Component)]
pub struct RigidBody {
    /// Kilograms; used for impulse response.
    ///
    /// C# struct defaults this to `0.0`. Callers must set mass explicitly for
    /// physics-driven bodies; entities without a meaningful mass should not be
    /// affected by physics.
    pub mass: f32,
    /// Decay rate in s^-1; `0.0` means no linear drag.
    pub linear_drag: f32,
    /// Decay rate in s^-1; `0.0` means no angular drag.
    pub angular_drag: f32,
    /// Moment of inertia in kg*px^2, computed at spawn in the source game.
    ///
    /// C# struct defaults this to `0.0`. Callers must set inertia explicitly
    /// for physics-driven bodies.
    pub inertia: f32,
    /// Reset after force integration.
    pub accumulated_force: Vec2,
    /// Reset after force integration.
    pub accumulated_torque: f32,
    /// Coefficient of restitution; default `0.0` means no bounce.
    pub restitution: f32,
    /// Coulomb friction coefficient; default `0.0` means frictionless contact.
    pub friction: f32,
    /// Seconds spent below sleep velocity thresholds.
    pub sleep_timer: f32,
    /// True when the body is at rest and skipped by physics until woken.
    pub asleep: bool,
}

impl Default for RigidBody {
    fn default() -> Self {
        Self {
            mass: 0.0,
            linear_drag: 0.0,
            angular_drag: 0.0,
            inertia: 0.0,
            accumulated_force: Vec2::ZERO,
            accumulated_torque: 0.0,
            restitution: 0.0,
            friction: 0.0,
            sleep_timer: 0.0,
            asleep: false,
        }
    }
}

/// Player entity marker.
///
/// Port of C# `Gameplay/Components/Tags.cs` `PlayerTag`.
#[derive(Component)]
pub struct PlayerTag;

/// Asteroid entity marker.
///
/// Port of C# `Gameplay/Components/Tags.cs` `AsteroidTag`.
#[derive(Component)]
pub struct AsteroidTag;

/// Alien entity marker.
///
/// Port of C# `Gameplay/Components/Tags.cs` `AlienTag`.
#[derive(Component)]
pub struct AlienTag;

/// Player bullet entity marker.
///
/// Port of C# `Gameplay/Components/Tags.cs` `BulletTag`.
#[derive(Component)]
pub struct BulletTag;

/// Alien bullet entity marker.
///
/// Port of C# `Gameplay/Components/Tags.cs` `AlienBulletTag`.
#[derive(Component)]
pub struct AlienBulletTag;

/// Key into `GameConfig.Asteroids`, e.g. `"standard"` or `"boulder"`.
///
/// Port of C# `Gameplay/Components/Tags.cs` `AsteroidVariant`.
#[derive(Component)]
pub struct AsteroidVariant {
    pub key: String,
}

/// Key into `GameConfig.Entities`, e.g. `"drone"` or `"mothership"`.
///
/// Port of C# `Gameplay/Components/Tags.cs` `AlienVariant`.
#[derive(Component)]
pub struct AlienVariant {
    pub key: String,
}

/// Asteroid type key used by editor/live material sync in the source game.
///
/// Port of C# `Gameplay/Components/Tags.cs` `AsteroidTypeKey`.
#[derive(Component)]
pub struct AsteroidTypeKey {
    pub key: String,
}

/// Entity exists but should be skipped by systems that opt into this filter.
///
/// Port of C# `Engine/Components/Tags.cs` `DisabledTag`. `DestroyTag` is not
/// ported because Bevy already handles despawn through `Commands`.
#[derive(Component)]
pub struct DisabledTag;
