//! Player ship spawning and simplified flight controls.

use bevy::{log::error, log::info, prelude::*};
use game_core::config::ShapeData;

use crate::{
    collision::game_layers,
    components::{PlayerTag, PreviousTransform, RigidBody, Velocity},
    config::{material_to_fracture_properties, GameConfigRes, ShapeLibrary},
    prefabs::{fracturable_body_collider, mass_and_inertia},
    systems::apply_force,
    FracturableBodyComp, PlayerInput,
};

const PLAYER_POS: Vec2 = Vec2::ZERO;
const PLAYER_RNG_SEED: u64 = 0x51c0_5eed;

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct AimComponent {
    pub dir: Vec2,
}

impl Default for AimComponent {
    fn default() -> Self {
        Self { dir: -Vec2::Y }
    }
}

pub(crate) fn spawn_player(
    mut commands: Commands,
    config: Res<GameConfigRes>,
    shapes: Res<ShapeLibrary>,
) {
    let player_config = &config.0.player;
    let shape_key = player_config.shape.as_str();
    let Some(shape) = shapes.0.get(shape_key) else {
        error!("player shape '{shape_key}' missing; skipping player spawn");
        return;
    };

    if shape.outline.len() < 3 || shape.seeds.is_empty() {
        error!(
            "player shape '{shape_key}' invalid: outline_vertices={} seeds={}; skipping player spawn",
            shape.outline.len(),
            shape.seeds.len()
        );
        return;
    }

    let Some(material) = resolve_player_material(&config, shape) else {
        error!("no materials configured; skipping player spawn");
        return;
    };

    let scale = player_config.shape_scale;
    let outline: Vec<Vec2> = shape
        .outline
        .iter()
        .map(|xy| Vec2::new(xy[0], xy[1]) * scale)
        .collect();
    let seed_positions: Vec<Vec2> = shape
        .seeds
        .iter()
        .map(|seed| Vec2::new(seed.x, seed.y) * scale)
        .collect();
    let seed_bond_mults: Vec<f32> = shape.seeds.iter().map(|seed| seed.bond_mult).collect();

    let mut rng = fracture::Rng::new(PLAYER_RNG_SEED);
    let mut body = fracture::build_from_explicit_seeds(
        &outline,
        &seed_positions,
        &seed_bond_mults,
        material,
        &mut rng,
    );
    apply_seed_density_mults(&mut body, shape, scale);

    let (mass, inertia) = mass_and_inertia(&body);
    let mut collider = fracturable_body_collider(&body);
    collider.layer = game_layers::PLAYER;
    collider.mask = game_layers::ASTEROID | game_layers::ALIEN;

    let cell_count = body.cells.len();
    commands.spawn((
        PlayerTag,
        AimComponent::default(),
        FracturableBodyComp(body),
        Transform::from_translation(PLAYER_POS.extend(0.5)),
        GlobalTransform::default(),
        PreviousTransform {
            position: PLAYER_POS,
            rotation: 0.0,
        },
        Velocity::default(),
        RigidBody {
            mass,
            inertia,
            linear_drag: 1.2,
            angular_drag: 2.0,
            restitution: 0.2,
            friction: 0.1,
            ..default()
        },
        collider,
    ));

    info!(
        "spawned player shape='{shape_key}' cells={} mass={:.3} inertia={:.3} thrust={:.3}",
        cell_count, mass, inertia, player_config.thrust
    );
}

pub(crate) fn aim_player(
    input: Res<PlayerInput>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    mut players: Query<(&mut Transform, &mut AimComponent), With<PlayerTag>>,
) {
    let Some(viewport_position) = input.aim_screen else {
        return;
    };
    let Ok((camera, camera_global_transform)) = camera.single() else {
        return;
    };
    let Ok(world_point) = camera.viewport_to_world_2d(camera_global_transform, viewport_position)
    else {
        return;
    };

    for (mut transform, mut aim) in &mut players {
        let dir = (world_point - transform.translation.truncate()).normalize_or_zero();
        if dir == Vec2::ZERO {
            continue;
        }

        aim.dir = dir;
        transform.rotation = Quat::from_rotation_z(dir.y.atan2(dir.x));
    }
}

pub(crate) fn move_player(
    input: Res<PlayerInput>,
    config: Res<GameConfigRes>,
    mut players: Query<&mut RigidBody, With<PlayerTag>>,
) {
    let thrust = input.thrust;
    if thrust == Vec2::ZERO {
        return;
    }

    let force = thrust * config.0.player.thrust;
    for mut body in &mut players {
        apply_force(&mut body, force);
    }
}

pub(crate) fn follow_player(
    players: Query<&Transform, (With<PlayerTag>, Without<Camera>)>,
    mut cameras: Query<&mut Transform, (With<Camera>, Without<PlayerTag>)>,
) {
    let Ok(player_transform) = players.single() else {
        return;
    };

    for mut camera_transform in &mut cameras {
        let z = camera_transform.translation.z;
        camera_transform.translation = player_transform.translation.truncate().extend(z);
    }
}

fn resolve_player_material(
    config: &GameConfigRes,
    shape: &ShapeData,
) -> Option<fracture::FractureProperties> {
    let override_key = config.0.player.material.trim();
    let shape_key = shape.material.trim();
    let material_config = (!override_key.is_empty())
        .then(|| config.0.materials.get(override_key))
        .flatten()
        .or_else(|| {
            (!shape_key.is_empty())
                .then(|| config.0.materials.get(shape_key))
                .flatten()
        })
        .or_else(|| config.0.materials.values().next())?;

    Some(material_to_fracture_properties(material_config))
}

fn apply_seed_density_mults(body: &mut fracture::FracturableBody, shape: &ShapeData, scale: f32) {
    for cell in &mut body.cells {
        let mut best_distance_sq = f32::MAX;
        let mut density_mult = 1.0;

        for seed in &shape.seeds {
            let seed_pos = Vec2::new(seed.x, seed.y) * scale;
            let distance_sq = cell.centroid.distance_squared(seed_pos);
            if distance_sq < best_distance_sq {
                best_distance_sq = distance_sq;
                density_mult = seed.density_mult;
            }
        }

        cell.density_mult = density_mult;
    }
}
