//! Rendering systems for the Bevy port.

use bevy::prelude::*;
use bevy_vector_shapes::prelude::*;

use crate::components::PreviousTransform;
use crate::systems::DemoMover;

const DEMO_MOVER_RADIUS: f32 = 15.0;
const DEMO_MOVER_COLOR: Color = Color::srgb(1.0, 0.55, 0.05);

pub(crate) fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

pub(crate) fn draw_demo_movers(
    mut painter: ShapePainter,
    fixed_time: Res<Time<Fixed>>,
    demo_movers: Query<(&Transform, &PreviousTransform), With<DemoMover>>,
) {
    let alpha = fixed_time.overstep_fraction();

    painter.hollow = false;
    painter.set_color(DEMO_MOVER_COLOR);

    for (transform, previous_transform) in &demo_movers {
        let current_position = transform.translation.truncate();
        let render_position = previous_transform.position.lerp(current_position, alpha);

        let current_rotation = transform.rotation.to_euler(EulerRot::ZYX).0;
        let render_rotation = lerp_angle(previous_transform.rotation, current_rotation, alpha);

        painter.set_translation(render_position.extend(transform.translation.z));
        painter.set_rotation(Quat::from_rotation_z(render_rotation));
        painter.circle(DEMO_MOVER_RADIUS);
    }
}

fn lerp_angle(previous: f32, current: f32, alpha: f32) -> f32 {
    let delta = (current - previous + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;

    previous + delta * alpha
}
