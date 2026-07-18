//! Fixed-step gameplay systems that operate on Bevy ECS components.

use bevy::{log::info, prelude::*};

use crate::{components::*, GameplayEntity};

#[derive(Component)]
pub(crate) struct DemoMover;

#[derive(Component)]
pub(crate) struct DemoForceMover;

/// Temporary Phase 3 movement probe; replace once Phase 5 spawns real gameplay
/// prefabs such as asteroids, ships, and bullets.
#[derive(Resource, Default)]
pub(crate) struct DemoMovementProbe {
    ticks: u64,
}

/// Temporary Phase 3 force/drag probe; remove when real thrust/impact systems
/// feed forces into physics-driven gameplay entities.
#[derive(Resource, Default)]
pub(crate) struct DemoForceProbe {
    ticks: u64,
}

/// Global acceleration field ported from C# `PhysicsSystem.Gravity`.
#[derive(Resource, Default, Deref, DerefMut)]
pub(crate) struct Gravity(pub Vec2);

/// Snapshot the current fixed-step pose before movement or physics mutates it.
pub(crate) fn previous_state_system(mut poses: Query<(&Transform, &mut PreviousTransform)>) {
    for (transform, mut previous) in &mut poses {
        previous.position = transform.translation.truncate();
        previous.rotation = transform.rotation.to_euler(EulerRot::ZYX).0;
    }
}

/// Integrate current pose from already-authored velocity. Force integration
/// belongs to the later physics system, not this kinematic movement pass.
pub(crate) fn movement_system(
    mut movers: Query<(&mut Transform, &Velocity)>,
    time: Res<Time<Fixed>>,
) {
    let dt = time.delta_secs();

    for (mut transform, velocity) in &mut movers {
        transform.translation += (velocity.linear * dt).extend(0.0);
        transform.rotate_z(velocity.angular * dt);
    }
}

/// Applies accumulated forces and drag to physics-driven bodies.
///
/// This is the velocity half of symplectic Euler. `movement_system` consumes
/// the updated velocity afterward to integrate position.
pub(crate) fn physics_system(
    mut bodies: Query<(&mut Velocity, &mut RigidBody)>,
    time: Res<Time<Fixed>>,
    gravity: Res<Gravity>,
) {
    let dt = time.delta_secs();

    for (mut velocity, mut body) in &mut bodies {
        if body.mass <= 0.0 || body.asleep {
            continue;
        }

        let mass = body.mass;
        let inertia = body.inertia;
        let linear_drag = body.linear_drag;
        let angular_drag = body.angular_drag;

        body.accumulated_force += **gravity * mass;

        velocity.linear += (body.accumulated_force / mass) * dt;
        if inertia > 0.0 {
            velocity.angular += (body.accumulated_torque / inertia) * dt;
        }

        velocity.linear *= (-linear_drag * dt).exp();
        velocity.angular *= (-angular_drag * dt).exp();

        body.accumulated_force = Vec2::ZERO;
        body.accumulated_torque = 0.0;
    }
}

pub(crate) fn apply_force(body: &mut RigidBody, force: Vec2) {
    body.asleep = false;
    body.sleep_timer = 0.0;
    body.accumulated_force += force;
}

pub(crate) fn apply_force_at_point(body: &mut RigidBody, force: Vec2, contact_offset: Vec2) {
    apply_force(body, force);
    body.accumulated_torque += contact_offset.x * force.y - contact_offset.y * force.x;
}

/// Temporary Phase 3 demo entities, proving fixed-step kinematic movement before
/// the real Phase 5 asteroid/player prefabs exist.
pub(crate) fn spawn_demo_movers(mut commands: Commands) {
    let movers = [
        (Vec2::new(-180.0, -80.0), Vec2::ZERO, 0.0),
        (Vec2::new(0.0, 40.0), Vec2::new(-28.0, 36.0), -1.20),
        (Vec2::new(160.0, -20.0), Vec2::new(16.0, -42.0), 0.55),
    ];

    for (index, (position, linear, angular)) in movers.into_iter().enumerate() {
        let mut entity = commands.spawn((
            GameplayEntity,
            DemoMover,
            Transform::from_translation(position.extend(0.0)),
            GlobalTransform::default(),
            Velocity { linear, angular },
            PreviousTransform::default(),
        ));

        if index == 0 {
            entity.insert((
                DemoForceMover,
                RigidBody {
                    mass: 12.0,
                    linear_drag: 0.3,
                    angular_drag: 0.4,
                    inertia: 80.0,
                    ..default()
                },
            ));
        }
    }

    info!(
        "spawned {} temporary Phase 3 demo movers, including one force/drag body",
        movers.len()
    );
}

/// Temporary Phase 3 force injection. Applies force for the first second of
/// fixed ticks so logs show acceleration, then drag-only decay.
pub(crate) fn apply_demo_force_probe(
    mut probe: ResMut<DemoForceProbe>,
    mut bodies: Query<&mut RigidBody, With<DemoForceMover>>,
) {
    probe.ticks += 1;

    if probe.ticks > 120 {
        return;
    }

    let Some(mut body) = bodies.iter_mut().next() else {
        return;
    };

    apply_force(&mut body, Vec2::new(180.0, 60.0));
    if probe.ticks == 1 {
        apply_force_at_point(&mut body, Vec2::new(0.0, 120.0), Vec2::new(18.0, 0.0));
        info!(
            "demo force probe: applying force for 120 fixed ticks, with one off-center torque kick"
        );
    }
}

/// Temporary Phase 3 verification probe. Logs the force-driven demo entity
/// sparingly so a headless run can confirm force, drag, and movement interact.
pub(crate) fn log_demo_movement_probe(
    mut probe: ResMut<DemoMovementProbe>,
    movers: Query<(&Transform, &PreviousTransform, &Velocity), With<DemoForceMover>>,
) {
    probe.ticks += 1;

    if probe.ticks % 60 != 0 {
        return;
    }

    let Some((transform, previous, velocity)) = movers.iter().next() else {
        return;
    };

    info!(
        "demo force/drag probe: tick={} position={:?} previous={:?} velocity={:?} angular_velocity={:.3} rotation={:.3} previous_rotation={:.3}",
        probe.ticks,
        transform.translation.truncate(),
        previous.position,
        velocity.linear,
        velocity.angular,
        transform.rotation.to_euler(EulerRot::ZYX).0,
        previous.rotation
    );
}
