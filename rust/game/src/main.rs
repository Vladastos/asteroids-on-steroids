//! Bevy glue. Shows HOW the pure `fracture` crate plugs into Bevy — the thin
//! system layer that replaces the C# `FractureService` (ECS half) +
//! `FractureCrackSystem` + `AsteroidSplitSystem`. All physics stays in the crate.

use std::time::Instant;

use bevy::{log::info, prelude::*};
use fracture::{
    build_result, compute_energy, count_components, drive_to_completion, seed_process,
    FracturableBody as PureBody, FractureInput, FractureProcess as PureProcess, Rng, WeaponProfile,
};

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
        .init_state::<AppState>()
        .insert_resource(Time::<Fixed>::from_seconds(1.0 / 120.0))
        .insert_resource(FixedTickProbe::default())
        .add_event::<ImpactEvent>()
        .add_systems(Startup, log_startup)
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
                main_menu_input.run_if(in_state(AppState::MainMenu)),
                playing_input.run_if(in_state(AppState::Playing)),
            ),
        )
        .add_systems(
            FixedUpdate,
            (seed_fractures, advance_fractures, log_fixed_tick_rate).chain(),
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
struct GameplayEntity;

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

/// Temporary Phase 2 timing probe; replace once real fixed-step gameplay exists.
#[derive(Resource)]
struct FixedTickProbe {
    ticks: u64,
    started: Instant,
}

impl Default for FixedTickProbe {
    fn default() -> Self {
        Self {
            ticks: 0,
            started: Instant::now(),
        }
    }
}

fn log_fixed_tick_rate(mut probe: ResMut<FixedTickProbe>) {
    probe.ticks += 1;

    if probe.ticks % 240 == 0 {
        let elapsed = probe.started.elapsed().as_secs_f64();
        info!(
            "fixed timestep probe: {} ticks in {:.2}s ({:.1} Hz)",
            probe.ticks,
            elapsed,
            probe.ticks as f64 / elapsed
        );
    }
}

/// The C# `FracturableBody` struct becomes a Bevy Component by wrapping the pure
/// type — the pure data does the physics, the Component makes it ECS-addressable.
#[derive(Component)]
struct FracturableBodyComp(PureBody);

/// A live multi-frame fracture (was the `FractureProcess` component in C#).
#[derive(Component)]
struct FractureProcessComp(PureProcess);

/// Minimal rigid-body data the fracture math needs (stand-in for RigidBody/Velocity).
#[derive(Component)]
struct Body {
    mass: f32,
    inertia: f32,
    linear: Vec2,
    angular: f32,
}

/// Replaces `CollisionEvent`-driven fracture dispatch (EventBus topic → Bevy event).
#[derive(Event)]
struct ImpactEvent {
    target: Entity,
    point: Vec2,
    dir: Vec2,
    normal_speed: f32,
    impactor_mass: f32,
}

/// On impact: compute energy (pure) and seed a `FractureProcess` component
/// (pure). Mirrors `FractureService.BeginFracture`'s fresh-process branch.
fn seed_fractures(
    mut commands: Commands,
    mut impacts: EventReader<ImpactEvent>,
    bodies: Query<(&FracturableBodyComp, &Transform, &Body), Without<FractureProcessComp>>,
) {
    for ev in impacts.read() {
        let Ok((body, xf, rb)) = bodies.get(ev.target) else {
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
            rb.mass,
            rb.inertia,
            body.0.material.restitution,
        );
        if e <= 0.0 && !body.0.fragile {
            continue;
        }
        let proc = seed_process(
            &body.0,
            -1,
            ev.point,
            pos,
            rot,
            rb.angular,
            ev.dir,
            e,
            &WeaponProfile::default(),
            ev.normal_speed,
        );
        commands.entity(ev.target).insert(FractureProcessComp(proc));
    }
}

/// Advance live fractures each fixed step; when a body splits, despawn it and
/// spawn its fragments. Mirrors `FractureCrackSystem` + `split_live`/`build_result`
/// + `AsteroidSplitSystem`. (Uses `drive_to_completion` for brevity; a real port
/// would step by each front's per-frame pacing and use `split_live` mid-crack.)
fn advance_fractures(
    mut commands: Commands,
    mut q: Query<(
        Entity,
        &mut FracturableBodyComp,
        &mut FractureProcessComp,
        &Transform,
        &Body,
    )>,
) {
    for (e, mut body, mut proc, xf, rb) in &mut q {
        drive_to_completion(&mut body.0, &mut proc.0);

        let n = body.0.cells.len();
        if count_components(n, &body.0.bonds, &proc.0.broken, &proc.0.pulverized) <= 1 {
            continue; // cracked but still one piece — keep the body, drop the process
        }

        let pos = xf.translation.truncate();
        let input = FractureInput {
            impact_point_world: proc.0.impact_point_world,
            impact_dir: proc.0.impact_dir,
            directionality: proc.0.directionality,
            blast_fraction: WeaponProfile::default().blast_fraction,
            body_position: pos,
            body_rotation: xf.rotation.to_euler(EulerRot::ZYX).0,
            body_linear: rb.linear,
            body_angular: rb.angular,
            body_mass: rb.mass,
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

        commands.entity(e).despawn();
        for frag in frags {
            // spawn_fragment(&mut commands, frag);  // was AsteroidPrefab.Create
            let _ = frag;
        }
    }
}
