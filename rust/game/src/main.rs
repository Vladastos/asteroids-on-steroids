//! Bevy glue. This file exists to show HOW the pure `fracture` crate plugs into
//! Bevy — the thin system layer that replaces the C# `FractureCrackSystem` /
//! `AsteroidSplitSystem`. Everything physical stays in the pure crate.

use bevy::prelude::*;
use fracture::{try_fracture, FracturableBody as PureBody, FractureResult, WeaponProfile};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // .add_plugins(bevy_vector_shapes::Shape2dPlugin::default())
        .add_event::<ImpactEvent>()
        .add_systems(FixedUpdate, apply_impacts)
        .run();
}

/// The C# `FracturableBody` struct becomes a Bevy Component by wrapping the pure
/// type. The pure data does the physics; the Component makes it ECS-addressable.
#[derive(Component)]
struct FracturableBodyComp(PureBody);

/// Replaces `CollisionEvent`-driven fracture dispatch — an EventBus topic → a Bevy event.
#[derive(Event)]
struct ImpactEvent {
    target: Entity,
    point: Vec2,
    dir: Vec2,
    normal_speed: f32,
    impactor_mass: f32,
}

/// THE GLUE, in full. Note: engine (pure crate) computes, system mutates the world
/// via `Commands` — the exact "engine spawns no entities" split from the C# design.
fn apply_impacts(
    mut commands: Commands,
    mut impacts: EventReader<ImpactEvent>,
    mut bodies: Query<(&mut FracturableBodyComp, &Transform)>,
) {
    for ev in impacts.read() {
        let Ok((mut body, xf)) = bodies.get_mut(ev.target) else { continue };

        let result: FractureResult = try_fracture(
            &mut body.0,
            ev.point,
            ev.dir,
            ev.normal_speed,
            ev.impactor_mass,
            &WeaponProfile::default(),
            xf.translation.truncate(),
            xf.rotation.to_euler(EulerRot::ZYX).0,
            Vec2::ZERO, // body linear  — read from a Velocity component in real code
            0.0,        // body angular
            1.0,        // body mass    — read from a RigidBody component
            1.0,        // body inertia
        );

        if result.fractured {
            commands.entity(ev.target).despawn();
            for frag in result.fragments {
                // spawn_fragment(&mut commands, frag);  // was AsteroidPrefab.Create
                let _ = frag;
            }
        }
    }
}
