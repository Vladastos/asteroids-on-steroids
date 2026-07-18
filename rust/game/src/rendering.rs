//! Rendering systems for the Bevy port.

use bevy::prelude::*;
use bevy_vector_shapes::prelude::*;

use crate::systems::DemoMover;

const DEMO_MOVER_RADIUS: f32 = 15.0;
const DEMO_MOVER_COLOR: Color = Color::srgb(1.0, 0.55, 0.05);

pub(crate) fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

pub(crate) fn draw_demo_movers(
    mut painter: ShapePainter,
    demo_movers: Query<&Transform, With<DemoMover>>,
) {
    painter.hollow = false;
    painter.set_color(DEMO_MOVER_COLOR);

    for transform in &demo_movers {
        painter.set_translation(transform.translation);
        painter.circle(DEMO_MOVER_RADIUS);
    }
}
