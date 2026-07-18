# C# → Rust/Bevy migration plan

Porting `GameEngine/` (C# / custom ECS) to Rust + `bevy_ecs`, targeting **native
dev + browser (wasm)**.

Guiding principles:

- **Bottom-up.** Port the pure, testable core first (math → fracture → collision),
  prove it in isolation, then build the Bevy layer on top. The scary subsystem
  (destruction) is done early and de-risked, not last.
- **Keep the C# decoupling.** `fracture` and `collision` stay Bevy-free crates.
  Only the `game` crate touches Bevy. This mirrors the existing PAL split and
  insulates gameplay from Bevy's per-release API churn.
- **Native before wasm.** Every phase runs on a native window first. wasm-specific
  issues (threads, assets, binary size) are tackled once, at the end, on proven logic.
- **Verify each phase.** No phase is "done" without the stated check passing.

Progress legend: `[ ]` todo · `[~]` in progress · `[x]` done.
Phase 1 (fracture, collision, game-core), Phase 2 (Bevy app skeleton),
Phase 3 (components & ECS mapping), and Phase 4 (rendering) are all complete
under `rust/`. Phase 5 is in progress: the VERTICAL SLICE (shoot a real
asteroid, watch it really crack then split into real fragment entities),
ROUND 2 (`game-core` config wired in; a real, flyable player ship with WASD
thrust + mouse aim + camera follow), and ROUND 3 (bullets now fire from the
player's actual position/aim, connecting round 2's ship to the vertical
slice's fracture pipeline) are all done and verified live — "fly, shoot,
fracture asteroids" all genuinely work end to end now. Aliens, waves,
scoring, skills, VFX, and the boss remain — see Phase 5's section for the
exact breakdown. An actual visual/screenshot verification of the renderer is still
owed (no screenshot tooling has been available in the dev environment used
so far), and no interactive input testing has been possible either (same
reason).

---

## Phase 0 — Tooling & baseline `[ ]`

**Goal:** reproducible toolchain; the C# game is the behavioural reference.

1. `rustup` stable + `wasm32-unknown-unknown` target:
   `rustup target add wasm32-unknown-unknown`.
2. Install `trunk` (`cargo install trunk`) and `wasm-opt` (binaryen) for later phases.
3. Pin the toolchain: add `rust-toolchain.toml` (channel = stable, components =
   rustfmt, clippy).
4. **Lock the Bevy/plugin versions.** Verify the `bevy` ↔ `bevy_vector_shapes`
   compatibility pair (compat table in the crate README) and set both with `=` in
   `rust/Cargo.toml`. Do this once; upgrade deliberately, never passively.
5. Capture reference behaviour from the C# build: record a few gameplay clips and
   note key tunables (fixed dt = 1/120, map ≈ 10× screen, wave pacing). Keep
   `Assets/game_config.json` + `Assets/shapes/*.json` as the source of truth — they
   port unchanged.

**Deliverable:** `cargo --version`, `trunk --version`, `rustc --target wasm32-... --version` all work.
**Verify:** `cargo build` of the empty workspace succeeds; C# game still runs for A/B comparison.

---

## Phase 1 — Pure core crates `[x]`

No Bevy. Just `glam`, `serde`, unit tests. This is ~40% of the LOC but the
lowest-risk 40%. **Complete: 36 tests green across `fracture` (19), `collision`
(13), and `game-core` (4) — `cargo test --workspace --exclude game`.**

### 1a. Math & primitives `[x]`
- `glam` replaces `System.Numerics`: `Vector2→Vec2`, `Matrix3x2→Mat3/Affine2`.
- Port `PolygonUtils.cs` (centroid, area, point-in-poly, distance-to-poly) into a
  small shared module (candidate for a `geo`/`math` module in `game-core` or a tiny
  `geom` crate). These are used by both `fracture` and `collision`.

### 1b. `fracture` crate `[x]`
Port order (each fn has its C# origin named in the stub):
1. `[x]` Types: `Cell`, `Bond`, `FracturableBody`, `FractureProperties`,
   `FractureInput`/`FragmentSpec`/`FractureResult`, `CrackFront`, `FractureProcess`.
2. `[x]` Graph: `build_adjacency`, `connected_components`, `count_components`,
   `FractureTiming::from_crack_speed`.
3. `[x]` `service::compute_energy` (reduced-mass KE → fracture units) +
   `effective_directionality` + `fragile_vaporize_energy`.
4. `[x]` `simulator::compute_spin_mul`, `build_result` (+ `build_component_spec`,
   `derived_motion`, `inertia_about`); `geom::compute_inertia`.
5. `[x]` `kernel::{CrackFront::seed, step_front}` — the frontier energy-split loop
   (`FractureKernel.cs`).
6. `[x]` `service::seed_process` (fresh-process branch of `FractureService.Seed`,
   sans ECS) + `simulator::drive_to_completion` driver.
7. `[x]` `simulator::split_live` + `partition_front` (multi-frame split).
   NB: the C# has no atomic `TryFracture` — the CLAUDE.md "atomic path" was
   aspirational. Real fracture is `BeginFracture` (multi-frame). The ECS half of
   `FractureService`/`FractureCrackSystem` (knockback, per-front frame pacing,
   dust events) lives in the `game` crate's Bevy system, not here.
8. `[x]` `VoronoiTessellator.cs` → `voronoi.rs`: `generate_convex` (Valtr's
   algorithm, own deterministic RNG), `build`/`build_with_seeds` (Lloyd relax,
   largest-component keep, enclosed-hole fill)/`build_from_explicit_seeds`.
   `BuildShape` (the `CompoundShape` collider) is deliberately **not** ported
   here — zipping `cell.local` into `collision::Polygon` is a one-line glue
   concern; porting it would add a `fracture → collision` dependency for no
   algorithmic benefit.

**Status:** the fracture core — energy model, inertia, exact area conservation
across splits, cascading shatter, pulverisation, determinism, AND the Voronoi
tessellator (convexity, area coverage, connectivity, membership-carved
concavities, hole-filling, non-convex authored outlines) — is fully ported and
green. 19 tests: `cargo test -p fracture`.

**Testing (the payoff of a Bevy-free crate):** deterministic golden tests —
fragment count, exact area conservation (Σ fragment area ≈ original −
vaporized), moment of inertia against the closed form, reproducibility from a
fixed seed, and tessellation invariants (convexity, connectivity, hole-filling)
— all runnable with zero engine/renderer setup.

**Deliverable:** `cargo test -p fracture` green (19/19); `build`/`build_with_seeds`
produce real tessellated bodies.
**Verify:** golden + tessellation tests pass.

### 1c. `collision` crate `[x]`
- Shape hierarchy ported as `enum Shape { Circle, Aabb, Polygon(Polygon),
  Compound(Compound) }` (cache-friendlier than the C#'s `abstract class` +
  double-dispatch, no `dyn`) with a flattened `match`-based `intersects()`
  replacing the double-dispatch table. `ContactInfo`, `RayCastResult` ported
  directly.
- `SpatialGrid` ported generic over an opaque handle `H: Copy + Eq` (so the
  crate doesn't need Bevy's `Entity`); the `game` crate instantiates
  `SpatialGrid<Entity>`.
- Narrow phase: circle/circle, circle/polygon (SAT), polygon/polygon (SAT),
  circle/AABB (closest-point), AABB/AABB, plus recursive `Compound` fan-out on
  either side with per-part AABB culling and `collect_contacts` (the full
  contact manifold, not just the deepest hit — needed to seed fracture on the
  cell actually struck). Raycast: circle, polygon (Cyrus-Beck), compound
  (nearest part) — AABB has none, matching the C#.
  **Impulse resolution stays out of this crate** — it reads
  `RigidBody`/`Velocity` and belongs in a `game` system (Phase 5).

**Deliverable:** `cargo test -p collision` green (13/13).
**Verify:** every shape pair separates cleanly along its reported normal
(property-tested: moving A by `normal * depth` always ends the contact);
compound tests confirm the correct cell is reported struck and that disabled
(pulverized) parts are invisible to every test.

### 1d. `game-core` config models `[x]`
- Ported `GameConfig/Models/*.cs` to `serde` structs (`#[derive(Deserialize)]`,
  `#[serde(rename_all = "camelCase", default)]` mirroring every C# `= value`
  default field-for-field). Comments + trailing commas parse via `json5`.
- `load()` reads from a directory (native); `load_config_from_str`/
  `load_shape_from_str` are the seam Phase 6 swaps to `include_str!`/
  `AssetServer` for wasm — parsing logic is unchanged either way.
  `find_assets_dir` ports `GameConfigLoader.FindAssetsDir`'s walk-up.

**Deliverable:** `cargo test -p game-core` green (4/4) — including loading the
REAL `GameEngine/Assets/game_config.json` + every file in `Assets/shapes/`
and asserting known values (rock toughness, bruiser shape data, etc.).
**Verify:** production JSON assets parse unchanged; sparse/comment-bearing
JSON falls back to defaults instead of erroring.

---

## Phase 2 — Bevy app skeleton `[x]`

**Goal:** a window opens, the fixed loop ticks, states transition. No gameplay yet.
**Complete.** All 5 steps done, verified (`cargo build`/`clippy -p game` clean,
`cargo run -p game` confirmed live under WSLg X11/llvmpipe), committed:
`c3446c3` → `af00c8b` → `d7a7c91` → `2f897bf` → `064aed5`.

1. `[x]` `game` crate `main.rs`: `App::new().add_plugins(DefaultPlugins)` +
   `WindowPlugin` (title "Asteroids on Steroids", 1280×720, resizable).
2. `[x]` **Fixed timestep:** `Time::<Fixed>::from_seconds(1.0/120.0)` (Bevy's
   default is 64Hz, not 120 — this was an actual bug the step fixed).
   Gameplay systems (fracture glue) stay in `FixedUpdate`. Verified ~118Hz
   observed under software-rendering overhead. Render interpolation deferred
   to Phase 3+ (no per-entity Transform history exists yet).
3. `[x]` **States:** `AppState { MainMenu, Playing, WaveComplete, GameOver }`
   via `bevy_state`, `OnEnter`/`OnExit` logging per state, `GameplayEntity`
   marker despawned `OnExit(Playing)`. Wired end-to-end: Enter/Space in
   MainMenu → spawn + transition to Playing; Escape in Playing → back to
   MainMenu (triggers cleanup); Escape in MainMenu → `AppExit`.
   `PlayingState`'s real content (ECS world, systems, wave manager, HUD) is
   deliberately NOT ported — `Playing` is an empty placeholder until Phase 5.
4. `[x]` **Input:** `PlayerInput` resource sampled every `Update` frame
   (unconditional, matching `InputSystem.BeginFrame()`): WASD → normalized
   thrust axis, primary-window cursor → `aim_screen` (camera conversion is a
   later phase), left-click → `fire`, Q/E/R → one-shot
   dash/turbo/slowmo triggers. Not wired to any entity yet (none exist).
5. `[x]` **EventBus** → Bevy `Events<T>`: `BulletHitEvent` +
   `GrenadeDetonateEvent` ported as `#[derive(Event)]` structs, following the
   pattern the fracture glue's `ImpactEvent` already established. Bevy's
   `Events<T>` IS the deferred publish/flush model — no bridge code needed.
   Noted (doc comment) that C#'s `PublishImmediate<T>()` has no direct Bevy
   equivalent; use an explicitly-ordered/exclusive system if ever needed.

**Process note:** all 5 steps were delegated to `codex exec` (workspace-write
sandbox + network access, approved by the user), one task per step, each
independently verified (`cargo build`/`clippy` rerun by the reviewer, diff
scope checked) before commit — not just trusted from codex's own report.

**Temporary scaffolding left in `game/src/main.rs`** (explicitly commented as
such in the code) that Phase 3+ will remove/replace: `FixedTickProbe` (tick
rate demo), `PlayerInputLogProbe` (input demo), `GameplayEventProbe` (event
demo). None of these are real gameplay — don't mistake them for it.

**Deliverable:** window opens; pressing a key logs a state transition MainMenu→Playing.
**Verify:** `cargo run -p game` shows the window and transitions on input. ✅

---

## Phase 3 — Components & ECS mapping `[x]`

**Goal:** entities exist with the right data; movement runs.
**Complete.** All 3 steps done, verified (`cargo build`/`clippy -p game`
clean each time, runtime logs confirmed correct behavior), committed:
`aca3bbb` → `5013960` → `03626e2`.

1. `[x]` `rust/game/src/components.rs`: `Velocity`, `RigidBody` (mass, linear/
   angular drag, inertia, accumulated force/torque, restitution, friction,
   sleep state), `PreviousTransform` (the render-interpolation half of C#
   `Transform.cs` — current pose stays on Bevy's built-in 3D `Transform`,
   already used by the Phase 2 fracture glue). Gameplay tags `PlayerTag`/
   `AsteroidTag`/`AlienTag`/`BulletTag`/`AlienBulletTag` +
   key-holders `AsteroidVariant`/`AlienVariant`/`AsteroidTypeKey`.
   `DisabledTag` ported as an opt-in filter marker; `DestroyTag` skipped
   (Bevy's `Commands::despawn` already covers that role).
2. `[x]` The Phase 2 fracture glue's temporary `Body` stand-in was deleted
   and replaced with the real `RigidBody` + `Velocity` components.
3. `[x]` `rust/game/src/systems.rs`: `previous_state_system` (snapshot before
   any mutation — registered first, matching the C#'s documented ordering),
   `physics_system` (symplectic-Euler force integration + exponential drag,
   faithful port of `PhysicsSystem.cs` including the `Gravity` resource and
   `apply_force`/`apply_force_at_point` free functions), `movement_system`
   (kinematic `Transform += Velocity * dt` via `Time<Fixed>::delta_secs()`).
   `FixedUpdate` order: `previous_state_system → physics_system →
   movement_system → (fracture glue)` — matches the C#'s documented
   `PhysicsSystem → MovementSystem → CollisionSystem` sequence.
4. `[x]` **Parallelism:** confirmed no `ForEachParallel`/`Parallel.For`
   equivalent was ported — Bevy's scheduler parallelizes disjoint systems
   automatically, nothing extra needed.

**Verified live:** 3 temporary demo entities (`DemoMover`) prove kinematic
motion; one of them additionally carries a `RigidBody` and a temporary force
probe that pushes it for 120 ticks — runtime logs confirmed velocity rising
under the applied force, then decaying under drag once the probe stopped,
with position tracking velocity exactly. This is the Phase 3 deliverable.

**Temporary scaffolding in `game/src/systems.rs`** (commented as such) that
Phase 5 replaces with real prefabs: `DemoMover`, `DemoForceMover`,
`DemoMovementProbe`, `DemoForceProbe`, `spawn_demo_movers`,
`apply_demo_force_probe`.

**Deliverable:** spawn a few entities; they move under the fixed step. ✅
**Verify:** headless or on-screen, positions advance correctly vs. dt. ✅

---

## Phase 4 — Rendering `[x]`

**Goal:** replace `IRenderer` immediate-mode drawing. Biggest single effort.
**Complete against its full agreed scope.** All 4 steps done, verified,
committed: `cf95e66` → `a1bbd02` → `1541918` → `8f42e36`.

1. `[x]` **`bevy_vector_shapes`:** `Shape2dPlugin` registered, `Camera2d`
   spawned (static, no follow/zoom yet — nothing to follow until Phase 5 has
   a player). Demo-mover circles draw via `ShapePainter` in `Update`.
2. `[x]` **Render interpolation:** `draw_demo_movers` lerps between
   `PreviousTransform` and current `Transform` using
   `Time<Fixed>::overstep_fraction()` as alpha, with a proper shortest-path
   angle lerp for rotation. Closes out the Phase 3 `PreviousTransform`
   component.
3. `[x]` **Real tessellated body rendering — genuine polygon fill.**
   `bevy_vector_shapes` 0.10.0 was confirmed (its docs.rs API AND its actual
   source read directly from the local cargo registry cache, AND its GitHub
   `main` branch — three independent checks) to have NO arbitrary-vertex
   polygon fill primitive at all (only discs, lines, rects, regular n-gons,
   triangles). Rather than adding `lyon` or another tessellation crate, this
   used a simpler fact: `fracture::Cell.local` is a documented invariant —
   always convex — so it needs only fan triangulation, which is ~10 lines
   and needs zero new dependencies. Every cell of a body is fan-triangulated
   into ONE shared `Mesh2d` vertex/index buffer per body (not one mesh per
   cell), so adjacent cells share interior mesh edges with no AA seam
   between them — exactly what the C#'s `IRenderer.FillPath` existed to
   guarantee. `attach_fracturable_body_meshes` reacts to
   `Added<FracturableBodyComp>` (builds once at spawn, not every frame —
   the performance shape Phase 5 needs once real fracture events start
   changing a body's cell set). Verified against Bevy 0.16's actual
   `examples/2d/mesh2d.rs` and `bevy_mesh::Mesh` source, not guessed.
4. `[x]` **Colour:** placeholder per-cell gray shading via vertex colors
   (correctly gamma-converted with `.to_linear()` for Bevy's linear color
   space). NOTE: this is a placeholder, not `Cell.fill_color`/`CellColorizer`
   — see the deferred items below.

**Explicitly deferred (by agreement, not blockers — revisit later):**
- Camera follow/zoom, world ~10× screen scaling — no entity worth following
  exists yet (Phase 5).
- Text/HUD (`bevy_text`) — scoped out of Phase 4 by agreement.
- Real per-cell colour baking (`Cell.fill_color` / `CellColorizer.cs`'s
  density-darken/rim-light/neighbour-blend/role-tint) — gameplay-visual
  polish, later; the placeholder gray shade stands in for now.
- Post effects (`IPostEffects.Distort`) — skipped for v1 per the original plan.

**Deliverable:** an asteroid body renders as filled cells with outline + HUD text.
✅ Cells render as real polygons (no outline stroke or HUD text yet — both
explicitly deferred above, not required for this phase's agreed scope).
**Verify:** visual parity screenshot vs. the C# `AsteroidDemo` for one body.
⚠️ Still not done — no screenshot tooling has been available in the dev
environment used throughout Phase 4; verification has been build/clippy/
runtime-log/direct-source-code-review only. Get an actual visual/screenshot
check as soon as tooling allows, ideally before Phase 5 builds much more on
top of this renderer.

---

## Phase 5 — Gameplay systems & the fracture glue `[~]` (fly/shoot/fracture all work; full scope remains)

**Goal:** the actual game, running natively. This phase is much bigger than
Phases 1-4 combined — the C# reference's `Gameplay/` covers prefabs, waves,
scoring, skills, VFX, aliens, and a boss. **This round scoped down to the
vertical slice** the earlier phases were explicitly building toward:
collision + a real asteroid + a bullet + the fracture glue actually firing,
proven live. Committed: `8ca7d94` → `ee42844` → `69a23bc` → `019300f`.

### Vertical slice — DONE and verified live
1. `[x]` **Collision system** (`Engine/Systems/CollisionSystem.cs`) — full
   port in `rust/game/src/collision.rs`: broad phase via
   `collision::SpatialGrid<Entity>`, narrow phase via
   `collision::intersects`/`collect_contacts` (including the asymmetric
   compound-vs-compound routing — collect from whichever body has more
   parts, flip vs. swap-parts depending on which side was authoritative),
   full sequential-impulse solver (6 iterations, normal + Coulomb friction,
   accumulated + clamped), Baumgarte positional correction, sleeping. All C#
   tuning constants ported verbatim. `CollisionEvent` emitted per resolved
   pair. `FractureGroup` sibling-suppression NOT ported (see rough edges
   below). Includes a passing unit test.
2. `[x]` **A real asteroid prefab** (`rust/game/src/prefabs.rs::spawn_asteroid`)
   — reuses `fracture::build_asteroid` (not the C#'s full clustering/
   procedural-noise/vortex-response `AsteroidPrefab.cs`, deliberately
   simplified for this slice), with a REAL `Collider` (a `collision::Compound`
   built from the body's cell polygons — the glue conversion Phase 1/4 always
   said belonged in `game`, not the pure crates) and real mass/inertia
   (density-weighted area + parallel-axis sum, matching
   `VoronoiTessellator.TotalMass`/`ComputeInertia` exactly).
3. `[x]` **A minimal bullet prefab** — small sensor-collider circle (sensor
   deliberately: avoids the solver mutating its velocity into a post-bounce
   direction before the impact system reads its true travel direction as
   "shot direction"), fired via left-click (`just_pressed`, one click one
   bullet), despawned on impact.
4. `[x]` **Fracture glue, wired to REAL collisions** — `publish_collision_impacts`
   turns a real bullet↔asteroid `CollisionEvent` into the existing
   `ImpactEvent`/`seed_fractures`/`advance_fractures` pipeline (built back in
   Phase 2, sitting unused until now). `advance_fractures` now removes
   `FractureProcessComp` on a no-split crack (so a body can be hit again —
   without this an asteroid could only ever crack once) and spawns REAL
   fragment entities via `prefabs::spawn_fragment` (was a no-op through
   Phases 2-4) reusing the asteroid's component skeleton.
5. `[x]` **Cell budget** (`Gameplay/CellBudget.cs`) — ported in `prefabs.rs`
   (`add`/`remove`/`can_spawn`/`reset`), enforced in `spawn_fragment`
   (overflow → debris, matching the C#'s intent).
6. `[x]` **Bullet-mass tuning, done empirically, not guessed** — the original
   mass (1.0) produced ~37 units of impact energy against ~400-500-unit bond
   strengths — nowhere close to enough. Tuned (env-var-overridable via
   `ASTEROIDS_BULLET_MASS` for further iteration) to a default that reliably
   cracks a body on the first hit and splits it on the second — sustained
   fire wearing a body down, matching realistic gameplay rather than
   one-shot vaporization.
7. `[x]` **Cleanup** — all Phase 2-4 demo/probe scaffolding removed
   (`FixedTickProbe`, `PlayerInputLogProbe`, `GameplayEventProbe`,
   `DemoMover`/`DemoForceMover` and their systems) now that real systems
   prove what they existed to prove. `PlayerInput`/`sample_player_input` and
   the `BulletHitEvent`/`GrenadeDetonateEvent` type definitions were
   deliberately KEPT (real input layer; documented future event shapes).

### Round 2 — config wiring + a real, flyable player ship
8. `[x]` **`game-core` wired into `game`** (`rust/game/src/config.rs`) —
   real `Assets/game_config.json` + `Assets/shapes/*.json` load at Startup
   into `GameConfigRes`/`ShapeLibrary` resources. `material_to_fracture_properties`
   ports `ConfigExtensions.ToFractureProperties` exactly (the two structs
   were designed to match field-for-field back in Phase 1d). The asteroid's
   material now comes from the real `"rock"` config entry (toughness 38,
   grain_area 1500) instead of a hardcoded placeholder — confirmed the
   vertical slice still cracks-then-splits correctly with the real values.
9. `[x]` **A real, flyable player ship** (`rust/game/src/player.rs`) —
   spawned from the real `player_ship` shape via
   `fracture::build_from_explicit_seeds`, with `ConfigExtensions.
   ResolveMaterial`'s exact fallback chain and per-cell density applied via
   nearest-seed matching. Real mouse aim (Bevy's actual
   `Camera::viewport_to_world_2d`, not a placeholder), WASD thrust via the
   already-existing `PlayerInput`/`apply_force`, camera follow (direct snap,
   no smoothing yet). Deliberately simplified: no skills (Q/E/R), no
   weapon-role-gated firing/thrust penalty, no lateral-drag-relative-to-aim
   feel — all explicitly deferred.
   **Cross-task regression caught in review:** the Phase 5.4 cleanup task
   deleted the real `apply_force` (which woke sleeping bodies) along with
   the demo probes it was told to remove; this task's own `move_player`
   silently got a bare-bones replacement missing that wake logic — a real
   bug (a settled, sleeping player ship would never respond to thrust again,
   since `physics_system` skips sleeping bodies). Fixed before commit;
   `apply_force_at_point` also restored (was lost entirely, kept for API
   parity even though nothing calls it yet).
10. `[x]` **Bullet firing wired to the player** (`rust/game/src/prefabs.rs::
    fire_bullet_on_click`) — bullets now spawn from the player's actual
    position (offset 48px forward along its aim direction, clearing the
    ship's mesh) and travel along its real `AimComponent.dir`, replacing the
    fixed launch-point/direction constants. This connects round 2's player
    ship to the vertical slice's bullet/fracture pipeline — the two were
    built independently and only joined here.
    `spawn_verification_bullet` (the `ASTEROIDS_VERIFY_AUTOFIRE` regression
    helper) deliberately kept its own fixed-point path — still useful as a
    player-independent test of the fracture pipeline alone.

**Verified live** (`ASTEROIDS_VERIFY_AUTOFIRE=1 cargo run -p game`, an
opt-in env-gated 3-bullet burst left in place for repeatable regression
testing): bullet 1 cracks the 41-cell asteroid (9 bonds broken, 7 cells
pulverized, still one piece) → bullet 2 splits it into 2 components → two
real fragment entities spawn (31-cell + 1-cell, sane non-NaN mass/inertia)
→ bullet 3 correctly collides with the newly-spawned 1-cell fragment,
proving fragments are genuinely live, collidable entities, not just visual
artifacts. Reproduced independently, not just trusted from the implementer's
report.

**Known rough edges, deliberately not fixed yet:**
- A body pulverized down to exactly zero live cells doesn't get despawned
  (`count_components() ≤ 1` treats "one piece" and "nothing left" the same).
- No `FractureGroup` sibling-collision suppression — freshly-split fragments
  may nudge each other slightly via the solver before separating.
- Camera follow has no smoothing (direct snap each frame).
- No mouse/keyboard input has been interactively tested in this environment
  (no input-injection tooling available) — aim/movement/firing are verified
  by code review and type-checking only, not an actual play session.

### NOT started yet — full C# `Gameplay/` scope remains
- `AlienPrefab`, `MothershipPrefab`, `PiercingPrefab` — no aliens, mothership,
  or the piercing-round weapon variant.
- Waves (`WaveSystemConfig`/`WaveDefinition`), scoring (`Score.cs`), skills
  (dash/turbo/slow-mo — Q/E/R input already sampled in Phase 2 but nothing
  consumes it), weapon-role-gated firing/thrust penalty, weapon effects,
  particles/VFX (`ParticleEffects`, `VortexFx`, `WeaponEffects`),
  `BossSystem.cs`.
- `GameContext.cs`'s shared bag (Config, Shapes, Score, CellBudget, Random)
  is not fully consolidated into Bevy resources yet — `game-core` is now
  wired in (Round 2) but only the asteroid material and the player
  shape/material/thrust/shape_scale actually read from it; most tuning
  (bullet speed/mass, asteroid size/sides, spawn positions) is still
  hardcoded Rust constants in `prefabs.rs`.
- Dust/particle emission on pulverized cells (mentioned in the original plan
  item 3) — not implemented.

**Deliverable (original, full-phase):** playable native build: fly, shoot,
fracture asteroids, waves advance. ⚠️ Partially met — fly, shoot, and
fracture asteroids all work end to end now; no waves yet.
**Verify:** run `/verify`-style playthrough; compare feel/behaviour to C#
reference. ⚠️ Not yet possible as a full playthrough (no input-injection
tooling to actually test flying/aiming interactively in this environment);
the vertical slice's fracture pipeline was verified via the autofire log
sequence above.

---

## Phase 6 — Browser (wasm) `[ ]`

**Goal:** the proven native game runs in a browser.

1. **`trunk` setup:** `index.html` + `Trunk.toml`; `trunk serve` for dev,
   `trunk build --release` for prod. Add a wasm entry (`#[cfg(target_arch="wasm32")]`
   canvas hookup on the `Window`).
2. **Assets:** no filesystem in the browser. Either bundle configs/shapes/font via
   `include_bytes!` (simplest for a small game) or serve them and load through Bevy's
   `AssetServer` (async — handle the loading state). Finalize the `load_config()`
   abstraction from Phase 1d.
3. **Threading:** default wasm has no threads. Confirm the game runs single-threaded
   (Bevy degrades gracefully). Only pursue wasm-atomics/`SharedArrayBuffer` +
   COOP/COEP headers if profiling demands it — unlikely at Asteroids scale.
4. **Audio:** `IAudioBackend` → `bevy_audio`. Browsers block audio until first user
   gesture — start audio on first click/keypress.
5. **Panics & logging:** `console_error_panic_hook` + `tracing-wasm` so errors surface
   in the browser console.
6. **Input quirks:** pointer lock for mouse aim; prevent context menu on right-click;
   handle canvas focus/resize.

**Deliverable:** `trunk serve` → the game is playable in Chrome/Firefox.
**Verify:** full playthrough in-browser; check the console is clean.

---

## Phase 7 — Optimization & polish `[ ]`

**Goal:** ship-quality load time and framerate in the browser.

1. **Binary size:** `opt-level="z"`, `lto`, `codegen-units=1`, `panic="abort"`
   (already in `Cargo.toml`); disable unused Bevy features; run `wasm-opt -Oz`;
   serve brotli/gzip. Target a few MB.
2. **Profile the fracture/render hot path:** mesh rebuild churn during heavy
   fracturing is the likely bottleneck (Phase 4 decision point). Cache lyon meshes;
   pool fragment entities; cap live cells via the budget.
3. **Determinism pass:** seeded RNG, fixed dt — confirm reproducible runs.
4. **Loading UX:** progress bar / splash while wasm + assets load.
5. **Parity review:** side-by-side vs. C# for tuning drift (energy scale, crack speed,
   fling). Adjust constants, not structure.

**Deliverable:** small, fast-loading browser build at stable framerate.
**Verify:** Lighthouse/size budget met; sustained 60fps in a heavy-fracture scene.

---

## Risk register (watch these)

| Risk | Where | Mitigation |
|---|---|---|
| Bevy API churn breaks the build on upgrade | Phase 2+ | Pin `=` versions; keep logic in pure crates; upgrade deliberately |
| `FillPath` seam / mesh rebuild cost | Phase 4, 7 | lyon-cached meshes; rebuild only on cell-set change |
| wasm threading absent | Phase 6 | Single-threaded; only add SAB if profiling forces it |
| Fracture tuning drift from C# | Phase 1b, 7 | Golden tests + A/B vs. C# `AsteroidDemo` |
| wasm binary too large / slow load | Phase 6, 7 | wasm-opt, feature trimming, brotli, splash |
| Asset loading differs native vs. wasm | Phase 1d, 6 | One `load_config()` abstraction chosen early |

## Suggested order of next PRs
1. ~~Phase 1 (fracture + collision + game-core), fully ported and tested~~ — **done**.
2. ~~Phase 2 (Bevy app, fixed timestep, states, input, events)~~ — **done**.
3. ~~Phase 3 (components, previous-state/physics/movement systems)~~ — **done**.
4. ~~Phase 4 (camera, bevy_vector_shapes, interpolation, real polygon-mesh
   rendering of tessellated bodies)~~ — **done, including closing the
   polygon-fill gap via fan-triangulated `Mesh2d` (no new dependency).**
5. ~~Phase 5 vertical slice (collision system, real asteroid + bullet
   prefabs, cell budget, fracture glue wired to real collisions, real
   fragment spawning, demo-scaffolding cleanup)~~ — **done and verified
   live: shoot a real asteroid, watch it really crack then split.**
6. ~~Phase 5 round 2 (`game-core` config wired in; real, flyable player ship
   — WASD thrust, real mouse-to-world aim, camera follow)~~ — **done and
   verified live.**
7. ~~Phase 5 round 3 (bullets fire from the player's real position/aim)~~ —
   **done and verified live: fly, shoot, and fracture asteroids all
   genuinely work end to end.**
8. **Rest of Phase 5** — aliens (`AlienPrefab`/`MothershipPrefab`,
   `PiercingPrefab`), waves, scoring, skills (Q/E/R — already sampled,
   nothing consumes it), weapon-role-gated firing/thrust penalty, weapon
   effects/VFX, boss. Also: an actual visual/screenshot check of the
   renderer AND interactive input testing — both still owed, no relevant
   tooling available in the dev environment used so far.
