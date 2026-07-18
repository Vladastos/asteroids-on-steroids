//! Gameplay prefab constructors for the Bevy-facing vertical slice.

use bevy::{log::info, prelude::*};
use fracture::{FracturableBody, FractureProperties, Rng};

use crate::{
    collision::game_layers,
    components::{AsteroidTag, BulletTag, Collider, PreviousTransform, RigidBody, Velocity},
    config::{material_to_fracture_properties, GameConfigRes},
    FracturableBodyComp, GameplayEntity,
};

pub(crate) const BULLET_RADIUS: f32 = 4.0;
pub(crate) const MAX_LIVE_CELLS: i32 = 600;

const ASTEROID_SIDES: usize = 12;
const ASTEROID_RADIUS: f32 = 80.0;
const ASTEROID_DRIFT: Vec2 = Vec2::new(-18.0, 6.0);
const ASTEROID_SPIN: f32 = 0.08;
const ASTEROID_POS: Vec2 = Vec2::new(360.0, 120.0);

const BULLET_LAUNCH_POS: Vec2 = Vec2::new(-520.0, 120.0);
const BULLET_SPEED: f32 = 900.0;
const DEFAULT_BULLET_MASS: f32 = 1000.0;
const BULLET_DIR: Vec2 = Vec2::X;

#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CellBudget {
    pub count: i32,
}

impl CellBudget {
    pub fn add(&mut self, n: i32) {
        self.count = self.count.saturating_add(n.max(0));
    }

    pub fn remove(&mut self, n: i32) {
        self.count = (self.count - n.max(0)).max(0);
    }

    pub fn can_spawn(&self, n: i32, max: i32) -> bool {
        self.count.saturating_add(n.max(0)) <= max
    }

    pub fn reset(&mut self) {
        self.count = 0;
    }
}

pub(crate) fn spawn_test_asteroid(
    mut commands: Commands,
    mut budget: ResMut<CellBudget>,
    config: Res<GameConfigRes>,
) {
    let mut rng = Rng::new(0x5eed);
    spawn_asteroid(&mut commands, &mut budget, &config, ASTEROID_POS, &mut rng);
}

pub(crate) fn spawn_verification_bullet(mut commands: Commands) {
    if std::env::var_os("ASTEROIDS_VERIFY_AUTOFIRE").is_none() {
        return;
    }

    for shot_index in 0..3 {
        let pos = BULLET_LAUNCH_POS - Vec2::X * 240.0 * shot_index as f32;
        let bullet = spawn_bullet(&mut commands, pos, BULLET_DIR);
        info!(
            "verification auto-fire spawned bullet={:?} shot={} mass={:.1} speed={:.1}",
            bullet,
            shot_index + 1,
            bullet_mass(),
            BULLET_SPEED
        );
    }
}

pub(crate) fn spawn_asteroid(
    commands: &mut Commands,
    budget: &mut CellBudget,
    config: &GameConfigRes,
    pos: Vec2,
    rng: &mut Rng,
) -> Entity {
    let material = asteroid_material(config);
    let body = fracture::build_asteroid(ASTEROID_SIDES, ASTEROID_RADIUS, material, None, rng);
    let (mass, inertia) = mass_and_inertia(&body);
    let collider = fracturable_body_collider(&body);
    let cell_count = body.cells.len();

    let entity = commands
        .spawn((
            AsteroidTag,
            FracturableBodyComp(body),
            Transform::from_translation(pos.extend(0.0)),
            GlobalTransform::default(),
            PreviousTransform {
                position: pos,
                rotation: 0.0,
            },
            Velocity {
                linear: ASTEROID_DRIFT,
                angular: ASTEROID_SPIN,
            },
            RigidBody {
                mass,
                inertia,
                ..asteroid_rigid_body_defaults()
            },
            collider,
        ))
        .id();

    budget.add(cell_count as i32);
    info!(
        "spawned asteroid cells={} mass={:.3} inertia={:.3} cell_budget={} material=rock toughness={:.3} grain_area={:.3}",
        cell_count, mass, inertia, budget.count, material.toughness, material.grain_area
    );

    entity
}

pub(crate) fn spawn_fragment(
    commands: &mut Commands,
    budget: &mut CellBudget,
    frag: &fracture::FragmentSpec,
) -> Option<Entity> {
    let cell_count = frag.body.cells.len() as i32;
    if frag.is_debris || !budget.can_spawn(cell_count, MAX_LIVE_CELLS) {
        info!(
            "fracture fragment converted to debris cells={} is_debris={} cell_budget={}",
            cell_count, frag.is_debris, budget.count
        );
        return None;
    }

    let entity = commands
        .spawn((
            AsteroidTag,
            FracturableBodyComp(frag.body.clone()),
            Transform {
                translation: frag.world_centroid.extend(0.0),
                rotation: Quat::from_rotation_z(frag.rotation),
                ..default()
            },
            GlobalTransform::default(),
            PreviousTransform {
                position: frag.world_centroid,
                rotation: frag.rotation,
            },
            Velocity {
                linear: frag.linear,
                angular: frag.angular,
            },
            RigidBody {
                mass: frag.mass,
                inertia: frag.inertia,
                ..asteroid_rigid_body_defaults()
            },
            fracturable_body_collider(&frag.body),
        ))
        .id();

    budget.add(cell_count);
    info!(
        "spawned fracture fragment entity={:?} cells={} mass={:.3} inertia={:.3} cell_budget={}",
        entity, cell_count, frag.mass, frag.inertia, budget.count
    );

    Some(entity)
}

pub(crate) fn fire_bullet_on_click(mut commands: Commands, mouse: Res<ButtonInput<MouseButton>>) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    spawn_bullet(&mut commands, BULLET_LAUNCH_POS, BULLET_DIR);
}

fn spawn_bullet(commands: &mut Commands, pos: Vec2, dir: Vec2) -> Entity {
    let shot_dir = dir.normalize_or_zero();

    commands
        .spawn((
            GameplayEntity,
            BulletTag,
            Transform::from_translation(pos.extend(1.0)),
            GlobalTransform::default(),
            PreviousTransform {
                position: pos,
                rotation: 0.0,
            },
            Velocity {
                linear: shot_dir * BULLET_SPEED,
                angular: 0.0,
            },
            RigidBody {
                mass: bullet_mass(),
                inertia: 1.0,
                linear_drag: 0.0,
                angular_drag: 0.0,
                restitution: 0.0,
                friction: 0.0,
                ..default()
            },
            Collider {
                shape: collision::Shape::Circle {
                    radius: BULLET_RADIUS,
                },
                layer: game_layers::BULLET,
                mask: game_layers::ASTEROID,
                sensor: true,
            },
        ))
        .id()
}

fn bullet_mass() -> f32 {
    std::env::var("ASTEROIDS_BULLET_MASS")
        .ok()
        .and_then(|mass| mass.parse::<f32>().ok())
        .filter(|mass| mass.is_finite() && *mass > 0.0)
        .unwrap_or(DEFAULT_BULLET_MASS)
}

fn asteroid_rigid_body_defaults() -> RigidBody {
    RigidBody {
        linear_drag: 0.05,
        angular_drag: 0.05,
        restitution: 0.3,
        friction: 0.2,
        ..default()
    }
}

fn asteroid_material(config: &GameConfigRes) -> FractureProperties {
    let material = config
        .0
        .materials
        .get("rock")
        .expect("game_config.json must define a 'rock' material");

    material_to_fracture_properties(material)
}

pub(crate) fn mass_and_inertia(body: &FracturableBody) -> (f32, f32) {
    let total_weighted_area: f32 = body
        .cells
        .iter()
        .map(|cell| cell.area * cell.density_mult)
        .sum();
    let mass = (total_weighted_area * body.material.density).max(1.0);

    if total_weighted_area <= 0.0 {
        return (mass, 0.0);
    }

    let inertia = body
        .cells
        .iter()
        .map(|cell| {
            let cell_mass = mass * (cell.area * cell.density_mult / total_weighted_area);
            fracture::compute_inertia(&cell.local, cell_mass)
                + cell_mass * cell.centroid.length_squared()
        })
        .sum();

    (mass, inertia)
}

pub(crate) fn fracturable_body_collider(body: &FracturableBody) -> Collider {
    let parts: Vec<_> = body
        .cells
        .iter()
        .filter(|cell| cell.local.len() >= 3)
        .map(|cell| collision::Shape::Polygon(collision::Polygon::new(cell.local.clone())))
        .collect();

    assert!(
        !parts.is_empty(),
        "fracturable asteroid must have at least one collider polygon"
    );

    Collider {
        shape: collision::Shape::Compound(collision::Compound::new(parts)),
        layer: game_layers::ASTEROID,
        mask: game_layers::ASTEROID | game_layers::PLAYER | game_layers::BULLET,
        sensor: false,
    }
}
