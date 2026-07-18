//! Fixed-step gameplay systems that operate on Bevy ECS components.

use bevy::prelude::*;

use crate::components::*;

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
