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
Phase 1 (all of it — fracture, collision, game-core) is complete under `rust/`.

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

## Phase 2 — Bevy app skeleton `[ ]`

**Goal:** a window opens, the fixed loop ticks, states transition. No gameplay yet.

1. `game` crate `main.rs`: `App::new().add_plugins(DefaultPlugins)`.
2. **Fixed timestep:** configure `Time<Fixed>` to 1/120 s; gameplay systems go in the
   `FixedUpdate` schedule (replaces `FixedTimestep.Advance`). Rendering/interpolation
   in `Update`.
3. **States:** port `GameCore/States/*` to `bevy_state`:
   `#[derive(States)] enum AppState { MainMenu, Playing, WaveComplete, GameOver }`.
   - `IGameState.Enter/Exit` → `OnEnter(state)` / `OnExit(state)` systems.
   - `IGameState.Update → IGameState?` (pointer swap) → set `NextState<AppState>`.
   - "Each PlayingState owns its World; teardown destroys entities" → tag gameplay
     entities with a marker component; `OnExit(Playing)` despawns all of them.
4. **Input:** `Engine/Input/InputSystem.cs` + `KeyCode.cs` → Bevy `ButtonInput<KeyCode>`
   / `ButtonInput<MouseButton>` + cursor position resource. Map WASD/mouse/Q-E-R/Esc.
5. **EventBus** → Bevy `Events<T>`: one `#[derive(Event)]` per current event type
   (`CollisionEvent`, gameplay events in `Gameplay/Events/`). `Publish→EventWriter`,
   `Subscribe→EventReader`. Bevy double-buffers events across frames — mind ordering
   vs. the old explicit `Flush()`.

**Deliverable:** window opens; pressing a key logs a state transition MainMenu→Playing.
**Verify:** `cargo run -p game` shows the window and transitions on input.

---

## Phase 3 — Components & ECS mapping `[ ]`

**Goal:** entities exist with the right data; movement runs.

1. Port `Engine/Components/*` and `Gameplay/Components/*` to
   `#[derive(Component)]` structs (glam types). `ref`-mutation → `&mut` in queries
   (Rust does this natively and more safely than C# `ref` returns).
2. Tags (`Engine/Components/Tags.cs`, `Gameplay/Components/Tags.cs`) → unit-struct
   marker components (`#[derive(Component)] struct Player;`).
3. Wrap pure-crate data for the ECS where needed:
   `#[derive(Component)] struct FracturableBodyComp(fracture::FracturableBody)`
   (already shown in `game/src/main.rs`). Keep the pure type unpolluted.
4. Port simple systems to verify the query model:
   - `MovementSystem.cs` / `PhysicsSystem.cs` → `FixedUpdate` systems over
     `Query<(&mut Transform, &Velocity)>`. `ForEach<T1,T2>` → `for .. in &mut query`.
   - `PreviousStateSystem.cs` → store prev transform for render interpolation.
5. **Parallelism:** do **not** port `ForEachParallel`/`Parallel.For`. Let Bevy's
   scheduler parallelize disjoint systems automatically. Accept single-threaded in
   wasm (Phase 6 note).

**Deliverable:** spawn a few entities; they move under the fixed step.
**Verify:** headless or on-screen, positions advance correctly vs. dt.

---

## Phase 4 — Rendering `[ ]`

**Goal:** replace `IRenderer` immediate-mode drawing. Biggest single effort.

1. **Primary path — `bevy_vector_shapes`:** immediate-mode filled polygons, lines,
   circles → maps closely to `DrawLine/FillPolygon/DrawCircle/FillCircle`. Add the
   `Shape2dPlugin`; draw in a system with the `ShapePainter`.
2. **`FillPath` (multi-contour, nonzero winding)** — the one primitive with no direct
   equivalent (used to draw a whole body's cells seamlessly). Two options:
   - Tessellate cells with `lyon` → one `Mesh2d` per body, rebuilt when the cell set
     changes (i.e. on fracture, not every frame). Best quality/perf.
   - Or draw each convex cell as a separate `bevy_vector_shapes` polygon and accept a
     faint AA seam (the C# `FillPath` doc explicitly exists to avoid that seam —
     decide if it matters).
3. **Camera:** port `Engine/Rendering/Camera.cs` to a `Camera2d` + transform; the
   world is ~10× screen, so drive camera follow + zoom. `PushTransform/PopTransform`
   (camera + per-entity) → entity `Transform` hierarchy, which Bevy handles.
4. **Text:** `DrawText/MeasureText/FontSpec` → `bevy_text` (`Text2d` for world,
   `Text` UI nodes for HUD). Not immediate-mode: spawn/update text entities. Bundle a
   `.ttf` (matching the current font) as an asset.
5. **Render interpolation:** the C# loop draws at sub-step `alpha`. Reproduce by
   lerping between `PreviousState` and current `Transform` in an `Update` system.
6. **Colour:** `Engine/Rendering/Color.cs` + `Cell.fill_color` (`Rgba`) →
   `bevy::Color`. Convert at draw time in the `game` crate (keeps `fracture` renderer-free).
7. **Post effects:** `IPostEffects.Distort` is optional/feature-detected — skip for v1
   (or later add a fullscreen shader material). The vortex/warp VFX are nice-to-have.

**Deliverable:** an asteroid body renders as filled cells with outline + HUD text.
**Verify:** visual parity screenshot vs. the C# `AsteroidDemo` for one body.

---

## Phase 5 — Gameplay systems & the fracture glue `[ ]`

**Goal:** the actual game, running natively.

1. **Prefabs** (`Gameplay/Prefabs/*`): static `Create(world, …)` factories →
   Rust `fn spawn_*(commands: &mut Commands, …) -> Entity` helpers (or Bevy `Bundle`s).
   `AsteroidPrefab`, `PlayerPrefab`, `AlienPrefab`, `MothershipPrefab`, `PiercingPrefab`.
2. **Collision system** (`Engine/Systems/CollisionSystem.cs`): broad phase (Phase 1c
   `SpatialGrid`) + narrow phase in a `FixedUpdate` system; on contact, apply impulse
   (needs `RigidBody`/`Velocity`) and emit `CollisionEvent`.
3. **Fracture glue** (the payoff — small): a system reads impact events, calls
   `fracture::try_fracture` / `begin_fracture`, and on `fractured` despawns the body +
   spawns `FragmentSpec`s via the prefab helper. See `game/src/main.rs::apply_impacts`
   — that pattern is the template. Multi-frame: store `FractureProcess` as a
   Component, advance with `split_live` each `FixedUpdate` (replaces
   `FractureCrackSystem`), emit dust on pulverized cells.
4. **Gameplay systems** (`Gameplay/Systems/*`, `BossSystem.cs`): waves
   (`WaveSystemConfig`/`WaveDefinition`), scoring (`Score.cs`), skills (dash/turbo/
   slow-mo — Q/E/R), weapon effects, particles/VFX (`ParticleEffects`, `VortexFx`,
   `WeaponEffects`). Port incrementally; each is an independent system.
5. **Cell budget** (`Gameplay/CellBudget.cs`): global cap on live cells → a Bevy
   `Resource`; enforce in the fracture-spawn system (convert overflow to debris).
6. **GameContext** (`Gameplay/GameContext.cs`): the shared bag (Config, Shapes, Score,
   CellBudget, Random) → split into Bevy `Resource`s. Use a seeded RNG resource
   (`rand` + a fixed seed) for determinism.

**Deliverable:** playable native build: fly, shoot, fracture asteroids, waves advance.
**Verify:** run `/verify`-style playthrough; compare feel/behaviour to C# reference.

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
2. **Phases 2–3** — Bevy app, states, components, movement (skeleton comes alive).
3. **Phase 4 minimal + Phase 5 fracture glue** — one asteroid you can shoot and shatter on screen (the vertical slice).
