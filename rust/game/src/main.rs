//! Bevy glue. Shows HOW the pure `fracture` crate plugs into Bevy — the thin
//! system layer that replaces the C# `FractureService` (ECS half) +
//! `FractureCrackSystem` + `AsteroidSplitSystem`. All physics stays in the crate.

use bevy::prelude::*;
use fracture::{
    build_result, compute_energy, count_components, drive_to_completion, seed_process,
    FracturableBody as PureBody, FractureInput, FractureProcess as PureProcess, Rng, WeaponProfile,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_event::<ImpactEvent>()
        .add_systems(FixedUpdate, (seed_fractures, advance_fractures).chain())
        .run();
}

/// The C# `FracturableBody` struct becomes a Bevy Component by wrapping the pure
/// type — the pure data does the physics, the Component makes it ECS-addressable.
#[derive(Component)]
struct FracturableBodyComp(PureBody);

/// A live multi-frame fracture (was the `FractureProcess` component in C#).
#[derive(Component)]
struct FractureProcessComp(PureProcess);

/// Minimal rigid-body data the fracture math needs (stand-in for RigidBody/Velocity).
#[derive(Component)]
struct Body {
    mass: f32,
    inertia: f32,
    linear: Vec2,
    angular: f32,
}

/// Replaces `CollisionEvent`-driven fracture dispatch (EventBus topic → Bevy event).
#[derive(Event)]
struct ImpactEvent {
    target: Entity,
    point: Vec2,
    dir: Vec2,
    normal_speed: f32,
    impactor_mass: f32,
}

/// On impact: compute energy (pure) and seed a `FractureProcess` component
/// (pure). Mirrors `FractureService.BeginFracture`'s fresh-process branch.
fn seed_fractures(
    mut commands: Commands,
    mut impacts: EventReader<ImpactEvent>,
    bodies: Query<(&FracturableBodyComp, &Transform, &Body), Without<FractureProcessComp>>,
) {
    for ev in impacts.read() {
        let Ok((body, xf, rb)) = bodies.get(ev.target) else { continue };
        let pos = xf.translation.truncate();
        let rot = xf.rotation.to_euler(EulerRot::ZYX).0;

        let e = compute_energy(
            ev.point, ev.dir, ev.normal_speed, ev.impactor_mass, pos, rb.mass, rb.inertia,
            body.0.material.restitution,
        );
        if e <= 0.0 && !body.0.fragile {
            continue;
        }
        let proc = seed_process(
            &body.0, -1, ev.point, pos, rot, rb.angular, ev.dir, e,
            &WeaponProfile::default(), ev.normal_speed,
        );
        commands.entity(ev.target).insert(FractureProcessComp(proc));
    }
}

/// Advance live fractures each fixed step; when a body splits, despawn it and
/// spawn its fragments. Mirrors `FractureCrackSystem` + `split_live`/`build_result`
/// + `AsteroidSplitSystem`. (Uses `drive_to_completion` for brevity; a real port
/// would step by each front's per-frame pacing and use `split_live` mid-crack.)
fn advance_fractures(
    mut commands: Commands,
    mut q: Query<(Entity, &mut FracturableBodyComp, &mut FractureProcessComp, &Transform, &Body)>,
) {
    for (e, mut body, mut proc, xf, rb) in &mut q {
        drive_to_completion(&mut body.0, &mut proc.0);

        let n = body.0.cells.len();
        if count_components(n, &body.0.bonds, &proc.0.broken, &proc.0.pulverized) <= 1 {
            continue; // cracked but still one piece — keep the body, drop the process
        }

        let pos = xf.translation.truncate();
        let input = FractureInput {
            impact_point_world: proc.0.impact_point_world,
            impact_dir: proc.0.impact_dir,
            directionality: proc.0.directionality,
            blast_fraction: WeaponProfile::default().blast_fraction,
            body_position: pos,
            body_rotation: xf.rotation.to_euler(EulerRot::ZYX).0,
            body_linear: rb.linear,
            body_angular: rb.angular,
            body_mass: rb.mass,
        };
        let mut rng = Rng::new(body.0.state.rng_seed as u64 | 1);
        let frags = build_result(
            &body.0, &input, &proc.0.broken, &proc.0.pulverized, &proc.0.fling_e, &mut rng, true,
        );

        commands.entity(e).despawn();
        for frag in frags {
            // spawn_fragment(&mut commands, frag);  // was AsteroidPrefab.Create
            let _ = frag;
        }
    }
}
