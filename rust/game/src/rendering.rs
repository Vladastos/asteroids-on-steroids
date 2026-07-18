//! Rendering systems for the Bevy port.

use bevy::prelude::*;
use bevy_vector_shapes::prelude::*;

use crate::components::PreviousTransform;
use crate::systems::DemoMover;
use crate::FracturableBodyComp;

const DEMO_MOVER_RADIUS: f32 = 15.0;
const DEMO_MOVER_COLOR: Color = Color::srgb(1.0, 0.55, 0.05);
const ASTEROID_BASE_COLOR: Vec3 = Vec3::new(0.42, 0.43, 0.43);

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

pub(crate) fn draw_fracturable_bodies(
    mut painter: ShapePainter,
    bodies: Query<(&Transform, &FracturableBodyComp)>,
) {
    painter.hollow = false;

    for (transform, body) in &bodies {
        let body_position = transform.translation.truncate();
        let body_rotation = transform.rotation.to_euler(EulerRot::ZYX).0;
        let z = transform.translation.z;

        for (index, cell) in body.0.cells.iter().enumerate() {
            let world_centroid = local_to_world(cell.centroid, body_position, body_rotation);
            painter.set_translation(world_centroid.extend(z));
            painter.set_rotation(Quat::IDENTITY);
            painter.set_color(cell_placeholder_color(index, cell.area));
            painter.circle((cell.area / std::f32::consts::PI).sqrt());
        }
    }
}

fn lerp_angle(previous: f32, current: f32, alpha: f32) -> f32 {
    let delta = (current - previous + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;

    previous + delta * alpha
}

fn local_to_world(local: Vec2, position: Vec2, rotation: f32) -> Vec2 {
    let (sin, cos) = rotation.sin_cos();
    Vec2::new(local.x * cos - local.y * sin, local.x * sin + local.y * cos) + position
}

fn cell_placeholder_color(index: usize, area: f32) -> Color {
    let index_shade = (index % 7) as f32 * 0.035;
    let area_shade = ((area / 600.0).clamp(0.0, 1.0) - 0.5) * 0.08;
    let shade = index_shade + area_shade;
    let rgb =
        (ASTEROID_BASE_COLOR + Vec3::splat(shade)).clamp(Vec3::splat(0.28), Vec3::splat(0.72));

    Color::srgb(rgb.x, rgb.y, rgb.z)
}
