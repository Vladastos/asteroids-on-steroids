//! Fixed-step gameplay systems that operate on Bevy ECS components.

use bevy::{log::info, prelude::*};

use crate::{components::*, GameplayEntity};

#[derive(Component)]
pub(crate) struct DemoMover;

/// Temporary Phase 3 movement probe; replace once Phase 5 spawns real gameplay
/// prefabs such as asteroids, ships, and bullets.
#[derive(Resource, Default)]
pub(crate) struct DemoMovementProbe {
    ticks: u64,
}

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

/// Temporary Phase 3 demo entities, proving fixed-step kinematic movement before
/// the real Phase 5 asteroid/player prefabs exist.
pub(crate) fn spawn_demo_movers(mut commands: Commands) {
    let movers = [
        (Vec2::new(-180.0, -80.0), Vec2::new(45.0, 18.0), 0.80),
        (Vec2::new(0.0, 40.0), Vec2::new(-28.0, 36.0), -1.20),
        (Vec2::new(160.0, -20.0), Vec2::new(16.0, -42.0), 0.55),
    ];

    for (position, linear, angular) in movers {
        commands.spawn((
            GameplayEntity,
            DemoMover,
            Transform::from_translation(position.extend(0.0)),
            GlobalTransform::default(),
            Velocity { linear, angular },
            PreviousTransform::default(),
        ));
    }

    info!("spawned {} temporary Phase 3 demo movers", movers.len());
}

/// Temporary Phase 3 verification probe. Logs one demo entity sparingly so a
/// headless run can confirm fixed-step movement is changing the pose.
pub(crate) fn log_demo_movement_probe(
    mut probe: ResMut<DemoMovementProbe>,
    movers: Query<(&Transform, &PreviousTransform), With<DemoMover>>,
) {
    probe.ticks += 1;

    if probe.ticks % 120 != 0 {
        return;
    }

    let Some((transform, previous)) = movers.iter().next() else {
        return;
    };

    info!(
        "demo movement probe: tick={} position={:?} previous={:?} rotation={:.3} previous_rotation={:.3}",
        probe.ticks,
        transform.translation.truncate(),
        previous.position,
        transform.rotation.to_euler(EulerRot::ZYX).0,
        previous.rotation
    );
}
