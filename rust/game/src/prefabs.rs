//! Gameplay prefab constructors for the Bevy-facing vertical slice.

use bevy::{log::info, prelude::*};
use fracture::{FracturableBody, FractureProperties, Rng};

use crate::{
    collision::game_layers,
    components::{AsteroidTag, BulletTag, Collider, PreviousTransform, RigidBody, Velocity},
    FracturableBodyComp, GameplayEntity,
};

pub(crate) const BULLET_RADIUS: f32 = 4.0;

const ASTEROID_SIDES: usize = 12;
const ASTEROID_RADIUS: f32 = 80.0;
const ASTEROID_DRIFT: Vec2 = Vec2::new(-18.0, 6.0);
const ASTEROID_SPIN: f32 = 0.08;
const ASTEROID_POS: Vec2 = Vec2::new(360.0, 120.0);

const BULLET_LAUNCH_POS: Vec2 = Vec2::new(-520.0, 120.0);
const BULLET_SPEED: f32 = 900.0;
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

pub(crate) fn spawn_test_asteroid(mut commands: Commands, mut budget: ResMut<CellBudget>) {
    let mut rng = Rng::new(0x5eed);
    spawn_asteroid(&mut commands, &mut budget, ASTEROID_POS, &mut rng);
}

pub(crate) fn spawn_asteroid(
    commands: &mut Commands,
    budget: &mut CellBudget,
    pos: Vec2,
    rng: &mut Rng,
) -> Entity {
    let body = fracture::build_asteroid(
        ASTEROID_SIDES,
        ASTEROID_RADIUS,
        asteroid_material(),
        None,
        rng,
    );
    let (mass, inertia) = mass_and_inertia(&body);
    let collider = asteroid_collider(&body);
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
                linear_drag: 0.05,
                angular_drag: 0.05,
                restitution: 0.3,
                friction: 0.2,
                ..default()
            },
            collider,
        ))
        .id();

    budget.add(cell_count as i32);
    info!(
        "spawned asteroid cells={} mass={:.3} inertia={:.3} cell_budget={}",
        cell_count, mass, inertia, budget.count
    );

    entity
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
                mass: 1.0,
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
                sensor: false,
            },
        ))
        .id()
}

fn asteroid_material() -> FractureProperties {
    FractureProperties {
        toughness: 24.0,
        restitution: 0.3,
        relax_rate: 100.0,
        brittleness: 0.55,
        crack_speed: 200.0,
        grain_area: 380.0,
        min_fragment_area: 100.0,
        density: 1.0,
        cell_toughness: 1.0,
        spin_pre_stress: 0.1,
        crack_directionality: 0.3,
        detach_cell_scale: 0.9,
        detach_cell_jitter: 0.02,
    }
}

fn mass_and_inertia(body: &FracturableBody) -> (f32, f32) {
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

fn asteroid_collider(body: &FracturableBody) -> Collider {
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
