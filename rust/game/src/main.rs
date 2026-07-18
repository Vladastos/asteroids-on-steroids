//! Bevy glue. Shows HOW the pure `fracture` crate plugs into Bevy — the thin
//! system layer that replaces the C# `FractureService` (ECS half) +
//! `FractureCrackSystem` + `AsteroidSplitSystem`. All physics stays in the crate.

use std::time::Instant;

use bevy::{log::info, prelude::*, window::PrimaryWindow};
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
        .insert_resource(GameplayEventProbe::default())
        .init_resource::<PlayerInput>()
        .init_resource::<PlayerInputLogProbe>()
        .add_event::<ImpactEvent>()
        .add_event::<BulletHitEvent>()
        .add_event::<GrenadeDetonateEvent>()
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
                sample_player_input,
                (
                    main_menu_input.run_if(in_state(AppState::MainMenu)),
                    playing_input.run_if(in_state(AppState::Playing)),
                    log_player_input_probe.run_if(in_state(AppState::Playing)),
                ),
                (publish_gameplay_event_probe, log_gameplay_event_probe).chain(),
            )
                .chain(),
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

/// Per-frame player input snapshot, mirroring the C# polling `InputSystem`.
///
/// `thrust` uses game-space axes: W is +Y, S is -Y, A is -X, D is +X.
/// Diagonal input is normalized so it does not exceed unit length.
#[derive(Resource, Debug, Clone, PartialEq)]
struct PlayerInput {
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

/// Temporary Phase 2 input probe; replace once Phase 5 wires this resource into
/// real player movement, weapons, and skills.
#[derive(Resource, Default)]
struct PlayerInputLogProbe {
    frames: u64,
    last_logged: Option<PlayerInput>,
}

fn log_player_input_probe(input: Res<PlayerInput>, mut probe: ResMut<PlayerInputLogProbe>) {
    probe.frames += 1;

    let changed = probe.last_logged.as_ref() != Some(&*input);
    if changed || probe.frames % 120 == 0 {
        info!("player input probe: {:?}", *input);
        probe.last_logged = Some(input.clone());
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

/// A bullet raycast struck a fracturable body.
///
/// Port of C# `BulletHitEvent`. `struck_cell` is `usize` to match the pure
/// fracture crate's cell indexing.
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
#[derive(Event)]
struct GrenadeDetonateEvent {
    grenade: Entity,
    world_pos: Vec2,
    weapon_key: String,
}

/// Temporary Phase 2 EventBus probe; remove once Phase 5 gameplay systems
/// publish real bullet and grenade events.
///
/// Bevy's `Events<T>` maps to the C# deferred `Publish<T>()` + per-frame
/// `Flush()` convention: a writer emits an event and later systems in the
/// schedule read it through `EventReader<T>`. If the C# `PublishImmediate<T>()`
/// pattern is ever truly needed, prefer an explicitly ordered/exclusive system
/// call, or accept Bevy's same-frame "next system in the schedule" latency.
#[derive(Resource, Default)]
struct GameplayEventProbe {
    published: bool,
}

fn publish_gameplay_event_probe(
    mut probe: ResMut<GameplayEventProbe>,
    mut bullet_hits: EventWriter<BulletHitEvent>,
    mut grenade_detonations: EventWriter<GrenadeDetonateEvent>,
) {
    if probe.published {
        return;
    }
    probe.published = true;

    // Placeholder entities make the event flow observable before Phase 5 adds
    // real bullets, grenades, and targets.
    let placeholder = Entity::PLACEHOLDER;
    bullet_hits.write(BulletHitEvent {
        target: placeholder,
        bullet: placeholder,
        struck_cell: 0,
        point: Vec2::ZERO,
        shot_dir: Vec2::Y,
    });
    grenade_detonations.write(GrenadeDetonateEvent {
        grenade: placeholder,
        world_pos: Vec2::ZERO,
        weapon_key: "phase2_probe_grenade".to_owned(),
    });
}

fn log_gameplay_event_probe(
    mut bullet_hits: EventReader<BulletHitEvent>,
    mut grenade_detonations: EventReader<GrenadeDetonateEvent>,
) {
    for ev in bullet_hits.read() {
        info!(
            "gameplay event probe: BulletHitEvent target={:?} bullet={:?} struck_cell={} point={:?} shot_dir={:?}",
            ev.target, ev.bullet, ev.struck_cell, ev.point, ev.shot_dir
        );
    }

    for ev in grenade_detonations.read() {
        info!(
            "gameplay event probe: GrenadeDetonateEvent grenade={:?} world_pos={:?} weapon_key={}",
            ev.grenade, ev.world_pos, ev.weapon_key
        );
    }
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
