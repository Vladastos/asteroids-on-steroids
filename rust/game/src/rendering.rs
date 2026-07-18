//! Rendering systems for the Bevy port.

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::mesh::{Indices, Mesh2d, PrimitiveTopology},
    sprite::{ColorMaterial, MeshMaterial2d},
};
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

pub(crate) fn attach_fracturable_body_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    bodies: Query<(Entity, &FracturableBodyComp), Added<FracturableBodyComp>>,
) {
    for (entity, body) in &bodies {
        commands.entity(entity).insert((
            Mesh2d(meshes.add(build_body_mesh(&body.0))),
            MeshMaterial2d(materials.add(ColorMaterial::default())),
        ));
    }
}

fn build_body_mesh(body: &fracture::FracturableBody) -> Mesh {
    let vertex_count = body.cells.iter().map(|cell| cell.local.len()).sum();
    let index_count = body
        .cells
        .iter()
        .map(|cell| cell.local.len().saturating_sub(2) * 3)
        .sum();
    let mut positions = Vec::with_capacity(vertex_count);
    let mut colors = Vec::with_capacity(vertex_count);
    let mut indices = Vec::with_capacity(index_count);

    for (cell_index, cell) in body.cells.iter().enumerate() {
        if cell.local.len() < 3 {
            continue;
        }

        let start_vertex =
            u32::try_from(positions.len()).expect("fracturable body mesh exceeds u32 vertices");
        let color = cell_mesh_color(cell_index, cell.area).to_linear().to_f32_array();

        for vertex in &cell.local {
            positions.push([vertex.x, vertex.y, 0.0]);
            colors.push(color);
        }

        for triangle_index in 1..cell.local.len() - 1 {
            let triangle_index =
                u32::try_from(triangle_index).expect("fracturable cell mesh exceeds u32 vertices");
            indices.extend_from_slice(&[
                start_vertex,
                start_vertex + triangle_index,
                start_vertex + triangle_index + 1,
            ]);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn lerp_angle(previous: f32, current: f32, alpha: f32) -> f32 {
    let delta = (current - previous + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;

    previous + delta * alpha
}

fn cell_mesh_color(index: usize, area: f32) -> Color {
    let index_shade = (index % 7) as f32 * 0.035;
    let area_shade = ((area / 600.0).clamp(0.0, 1.0) - 0.5) * 0.08;
    let shade = index_shade + area_shade;
    let rgb =
        (ASTEROID_BASE_COLOR + Vec3::splat(shade)).clamp(Vec3::splat(0.28), Vec3::splat(0.72));

    Color::srgb(rgb.x, rgb.y, rgb.z)
}
