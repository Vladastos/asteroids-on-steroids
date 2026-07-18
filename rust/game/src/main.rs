//! Bevy glue. Shows HOW the pure `fracture` crate plugs into Bevy — the thin
//! system layer that replaces the C# `FractureService` (ECS half) +
//! `FractureCrackSystem` + `AsteroidSplitSystem`. All physics stays in the crate.

pub mod collision;
pub mod components;
pub mod config;
pub mod player;
pub mod prefabs;
pub mod rendering;
pub mod systems;

use crate::collision::*;
use bevy::{log::info, prelude::*, window::PrimaryWindow};
use bevy_vector_shapes::prelude::*;
use components::*;
use config::*;
use fracture::{
    build_result, compute_energy, count_components, drive_to_completion, seed_process,
    FracturableBody as PureBody, FractureInput, FractureProcess as PureProcess, Rng, WeaponProfile,
};
use player::*;
use prefabs::*;
use rendering::*;
use systems::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Asteroids on Steroids".into(),
                resolution: (1280., 720.).into(),
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(Shape2dPlugin::default())
        .init_state::<AppState>()
        .insert_resource(Time::<Fixed>::from_seconds(1.0 / 120.0))
        .init_resource::<Gravity>()
        .init_resource::<PlayerInput>()
        .init_resource::<CollisionGrid>()
        .init_resource::<CellBudget>()
        .add_event::<ImpactEvent>()
        .add_event::<BulletHitEvent>()
        .add_event::<GrenadeDetonateEvent>()
        .add_event::<CollisionEvent>()
        .add_systems(
            Startup,
            (
                load_game_config,
                log_startup,
                spawn_camera,
                spawn_player,
                spawn_test_asteroid,
                spawn_verification_bullet,
            )
                .chain(),
        )
        .add_systems(OnEnter(AppState::MainMenu), enter_main_menu)
        .add_systems(OnExit(AppState::MainMenu), exit_main_menu)
        .add_systems(OnEnter(AppState::Playing), enter_playing)
        .add_systems(
            OnExit(AppState::Playing),
            (exit_playing, cleanup_gameplay_entities).chain(),
        )
        .add_systems(OnEnter(AppState::WaveComplete), enter_wave_complete)
        .add_systems(OnExit(AppState::WaveComplete), exit_wave_complete)
        .add_systems(OnEnter(AppState::GameOver), enter_game_over)
        .add_systems(OnExit(AppState::GameOver), exit_game_over)
        .add_systems(
            Update,
            (
                sample_player_input,
                aim_player,
                follow_player,
                (
                    main_menu_input.run_if(in_state(AppState::MainMenu)),
                    playing_input.run_if(in_state(AppState::Playing)),
                    fire_bullet_on_click.run_if(in_state(AppState::Playing)),
                ),
                draw_bullets,
                attach_fracturable_body_meshes,
            )
                .chain(),
        )
        .add_systems(
            FixedUpdate,
            (
                previous_state_system,
                move_player,
                physics_system,
                movement_system,
                collision_system,
                publish_collision_impacts,
                seed_fractures,
                advance_fractures,
            )
                .chain(),
        )
        .run();
}

fn log_startup() {
    info!("Asteroids on Steroids Bevy app started");
}

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
enum AppState {
    #[default]
    MainMenu,
    Playing,
    WaveComplete,
    GameOver,
}

#[derive(Component)]
pub(crate) struct GameplayEntity;

fn enter_main_menu() {
    info!("Entering MainMenu");
}

fn exit_main_menu() {
    info!("Exiting MainMenu");
}

fn enter_playing() {
    info!("Entering Playing");
}

fn exit_playing() {
    info!("Exiting Playing");
}

fn enter_wave_complete() {
    info!("Entering WaveComplete");
}

fn exit_wave_complete() {
    info!("Exiting WaveComplete");
}

fn enter_game_over() {
    info!("Entering GameOver");
}

fn exit_game_over() {
    info!("Exiting GameOver");
}

fn main_menu_input(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut app_exit: EventWriter<AppExit>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        info!("MainMenu input: Escape pressed, quitting");
        app_exit.write(AppExit::Success);
        return;
    }

    if keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::Space) {
        info!("MainMenu input: starting placeholder Playing state");
        commands.spawn(GameplayEntity);
        next_state.set(AppState::Playing);
    }
}

fn playing_input(keyboard: Res<ButtonInput<KeyCode>>, mut next_state: ResMut<NextState<AppState>>) {
    if keyboard.just_pressed(KeyCode::Escape) {
        info!("Playing input: returning to MainMenu");
        next_state.set(AppState::MainMenu);
    }
}

fn cleanup_gameplay_entities(
    mut commands: Commands,
    gameplay_entities: Query<Entity, With<GameplayEntity>>,
) {
    for entity in &gameplay_entities {
        commands.entity(entity).despawn();
    }
}

/// Per-frame player input snapshot, mirroring the C# polling `InputSystem`.
///
/// `thrust` uses game-space axes: W is +Y, S is -Y, A is -X, D is +X.
/// Diagonal input is normalized so it does not exceed unit length.
#[allow(dead_code)]
#[derive(Resource, Debug, Clone, PartialEq)]
pub(crate) struct PlayerInput {
    thrust: Vec2,
    /// Primary-window cursor position in screen pixels. A later gameplay phase
    /// will convert this through the camera once world entities exist.
    aim_screen: Option<Vec2>,
    fire: bool,
    skill_dash: bool,
    skill_turbo: bool,
    skill_slowmo: bool,
}

impl Default for PlayerInput {
    fn default() -> Self {
        Self {
            thrust: Vec2::ZERO,
            aim_screen: None,
            fire: false,
            skill_dash: false,
            skill_turbo: false,
            skill_slowmo: false,
        }
    }
}

fn sample_player_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    mut player_input: ResMut<PlayerInput>,
) {
    let mut thrust = Vec2::ZERO;
    if keyboard.pressed(KeyCode::KeyW) {
        thrust.y += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        thrust.y -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) {
        thrust.x -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        thrust.x += 1.0;
    }
    if thrust.length_squared() > 1.0 {
        thrust = thrust.normalize();
    }

    *player_input = PlayerInput {
        thrust,
        aim_screen: primary_window
            .single()
            .ok()
            .and_then(Window::cursor_position),
        fire: mouse.pressed(MouseButton::Left),
        skill_dash: keyboard.just_pressed(KeyCode::KeyQ),
        skill_turbo: keyboard.just_pressed(KeyCode::KeyE),
        skill_slowmo: keyboard.just_pressed(KeyCode::KeyR),
    };
}

/// The C# `FracturableBody` struct becomes a Bevy Component by wrapping the pure
/// type — the pure data does the physics, the Component makes it ECS-addressable.
#[derive(Component)]
struct FracturableBodyComp(PureBody);

/// A live multi-frame fracture (was the `FractureProcess` component in C#).
#[derive(Component)]
struct FractureProcessComp(PureProcess);

/// Replaces `CollisionEvent`-driven fracture dispatch (EventBus topic → Bevy event).
#[derive(Event)]
struct ImpactEvent {
    target: Entity,
    point: Vec2,
    dir: Vec2,
    normal_speed: f32,
    impactor_mass: f32,
}

/// A bullet raycast struck a fracturable body.
///
/// Port of C# `BulletHitEvent`. `struck_cell` is `usize` to match the pure
/// fracture crate's cell indexing.
#[allow(dead_code)]
#[derive(Event)]
struct BulletHitEvent {
    target: Entity,
    bullet: Entity,
    struck_cell: usize,
    point: Vec2,
    shot_dir: Vec2,
}

/// A grenade reached its fuse end or hit something and should detonate.
///
/// Port of C# `GrenadeDetonateEvent`. `weapon_key` stays a `String` because
/// authored weapon ids are data-driven strings in the source game; later phases
/// can intern or enum-ize them once the real weapon catalog is ported.
#[allow(dead_code)]
#[derive(Event)]
struct GrenadeDetonateEvent {
    grenade: Entity,
    world_pos: Vec2,
    weapon_key: String,
}

fn publish_collision_impacts(
    mut commands: Commands,
    mut collisions: EventReader<CollisionEvent>,
    bullets: Query<(&Velocity, &RigidBody), With<BulletTag>>,
    asteroids: Query<(), With<AsteroidTag>>,
    mut impacts: EventWriter<ImpactEvent>,
) {
    for ev in collisions.read() {
        let (bullet, asteroid, bullet_velocity, bullet_body) =
            match (bullets.get(ev.entity_a), asteroids.get(ev.entity_b)) {
                (Ok((velocity, rigid_body)), Ok(())) => {
                    (ev.entity_a, ev.entity_b, velocity, rigid_body)
                }
                _ => match (bullets.get(ev.entity_b), asteroids.get(ev.entity_a)) {
                    (Ok((velocity, rigid_body)), Ok(())) => {
                        (ev.entity_b, ev.entity_a, velocity, rigid_body)
                    }
                    _ => continue,
                },
            };

        let shot_dir = bullet_velocity.linear.normalize_or_zero();
        impacts.write(ImpactEvent {
            target: asteroid,
            point: ev.contact.contact_point,
            dir: shot_dir,
            normal_speed: ev.approach_speed,
            impactor_mass: bullet_body.mass,
        });
        info!(
            "collision impact: bullet={:?} asteroid={:?} point={:?} dir={:?} speed={:.3} mass={:.3}",
            bullet, asteroid, ev.contact.contact_point, shot_dir, ev.approach_speed, bullet_body.mass
        );
        commands.entity(bullet).despawn();
    }
}

/// On impact: compute energy (pure) and seed a `FractureProcess` component
/// (pure). Mirrors `FractureService.BeginFracture`'s fresh-process branch.
fn seed_fractures(
    mut commands: Commands,
    mut impacts: EventReader<ImpactEvent>,
    bodies: Query<
        (&FracturableBodyComp, &Transform, &RigidBody, &Velocity),
        Without<FractureProcessComp>,
    >,
) {
    for ev in impacts.read() {
        let Ok((body, xf, rigid_body, velocity)) = bodies.get(ev.target) else {
            continue;
        };
        let pos = xf.translation.truncate();
        let rot = xf.rotation.to_euler(EulerRot::ZYX).0;

        let e = compute_energy(
            ev.point,
            ev.dir,
            ev.normal_speed,
            ev.impactor_mass,
            pos,
            rigid_body.mass,
            rigid_body.inertia,
            body.0.material.restitution,
        );
        if e <= 0.0 && !body.0.fragile {
            continue;
        }
        info!(
            "seed fracture: target={:?} energy={:.3} normal_speed={:.3} impactor_mass={:.3}",
            ev.target, e, ev.normal_speed, ev.impactor_mass
        );
        let proc = seed_process(
            &body.0,
            -1,
            ev.point,
            pos,
            rot,
            velocity.angular,
            ev.dir,
            e,
            &WeaponProfile::default(),
            ev.normal_speed,
        );
        commands.entity(ev.target).insert(FractureProcessComp(proc));
    }
}

/// Advance live fractures each fixed step; when a body splits, despawn it and
/// spawn its fragments. Mirrors `FractureCrackSystem`,
/// `split_live`/`build_result`, and `AsteroidSplitSystem`. (Uses
/// `drive_to_completion` for brevity; a real port
/// would step by each front's per-frame pacing and use `split_live` mid-crack.)
fn advance_fractures(
    mut commands: Commands,
    mut q: Query<(
        Entity,
        &mut FracturableBodyComp,
        &mut FractureProcessComp,
        &Transform,
        &RigidBody,
        &Velocity,
    )>,
    mut budget: ResMut<CellBudget>,
) {
    for (e, mut body, mut proc, xf, rigid_body, velocity) in &mut q {
        drive_to_completion(&mut body.0, &mut proc.0);

        let n = body.0.cells.len();
        let component_count =
            count_components(n, &body.0.bonds, &proc.0.broken, &proc.0.pulverized);
        if component_count <= 1 {
            let broken_count = proc.0.broken.iter().filter(|&&broken| broken).count();
            let pulverized_count = proc
                .0
                .pulverized
                .iter()
                .filter(|&&pulverized| pulverized)
                .count();
            info!(
                "fracture completed without split: entity={:?} cells={} broken_bonds={} pulverized_cells={}",
                e, n, broken_count, pulverized_count
            );
            commands.entity(e).remove::<FractureProcessComp>();
            continue;
        }
        info!(
            "fracture split detected: entity={:?} cells={} components={}",
            e, n, component_count
        );

        let pos = xf.translation.truncate();
        let input = FractureInput {
            impact_point_world: proc.0.impact_point_world,
            impact_dir: proc.0.impact_dir,
            directionality: proc.0.directionality,
            blast_fraction: WeaponProfile::default().blast_fraction,
            body_position: pos,
            body_rotation: xf.rotation.to_euler(EulerRot::ZYX).0,
            body_linear: velocity.linear,
            body_angular: velocity.angular,
            body_mass: rigid_body.mass,
        };
        let mut rng = Rng::new(body.0.state.rng_seed as u64 | 1);
        let frags = build_result(
            &body.0,
            &input,
            &proc.0.broken,
            &proc.0.pulverized,
            &proc.0.fling_e,
            &mut rng,
            true,
        );

        budget.remove(body.0.cells.len() as i32);
        info!(
            "despawning fractured body entity={:?} removed_cells={} fragment_specs={} cell_budget={}",
            e,
            body.0.cells.len(),
            frags.len(),
            budget.count
        );
        commands.entity(e).despawn();
        for frag in frags {
            prefabs::spawn_fragment(&mut commands, &mut budget, &frag);
        }
    }
}
