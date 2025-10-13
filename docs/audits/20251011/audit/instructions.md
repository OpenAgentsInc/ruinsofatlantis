Below is a concrete, *phase‑one* refactor playbook you can hand to a coding agent. It is designed to be executed in small, reviewable PRs without changing behavior. It focuses on carving out modules, introducing thin typed wrappers, tightening feature gates, and adding lightweight tests/docs—**no algorithmic or rendering logic changes**.

---

## Ground rules (apply to every PR)

* **No behavior changes.** Only structural moves, newtypes, and re‑exports. Keep public APIs source‑compatible via shims.
* **Small, stacked PRs.** Each < ~400 LOC net diff where possible; one theme per PR. Land when green.
* **Re‑export for compatibility.** If you move/rename types, add `pub use` aliases from the old path.
* **Zero default feature bloat.** Demo paths remain behind features; defaults stay minimal.
* **Docs & tests.** Add brief rustdoc for new public structs and at least one CPU‑only unit test per new module.
* **CI stays green.** Run `cargo xtask ci` locally after each PR; fix clippy warnings (deny-by-default).

---

## Preflight (once)

1. Read `docs/architecture/ECS_ARCHITECTURE_GUIDE.md` and `crates/README.md`.
2. Create branch: `git checkout -b refactor/phase1-skeleton`.
3. Establish a baseline:

   * `cargo xtask ci`
   * `rg --stats "pub struct Renderer"` and list current public fields (snapshot in a markdown note committed under `devnotes/`).
   * Identify feature flags in `Cargo.toml` across affected crates.

---

## Phase‑one objectives (what “done” looks like)

* Monolith files (≥1000 LOC) are split into small modules with **no logic change**.
* A **device/surface context** (`GpuCtx`, `SurfaceCtx`) and **attachments** holder exist and are owned by `Renderer`.
* Per‑pipeline files exist under `renderer::pipelines::*` with thin typed wrappers; legacy paths re‑exported.
* A minimal **framegraph skeleton** (`renderer::graph`) exists that simply forwards to the current render flow (no reordering yet).
* Demo/experimental code is moved behind explicit features and out of hot paths.
* `tools/model-viewer` and `platform_winit` are split by concern with unchanged behavior.
* `server_core::ecs::schedule` has stage files and a `system_names_for_test()` helper (order unchanged).
* `net_core::snapshot` split by domain; encode helpers isolated.
* `server_core::lib.rs` shrinks to re‑exports + top‑level docs.

---

## Global file layout to create (all stubs compile)

> **Do not delete the original modules yet;** first move types/functions, add re‑exports from old paths, then in a later PR you can remove temporary shims.

```
crates/render_wgpu/src/gfx/
  renderer/
    device.rs          // GpuCtx, SurfaceCtx, Samplers
    attachments.rs     // Attachments (color, depth, msaa)
    config.rs          // renderer toggles & constants
    graph.rs           // framegraph skeleton & pass stubs
    drawlists.rs       // draw grouping/types (moved only)
    update/
      mod.rs
      projectiles.rs
      builder.rs
      destructibles_demo.rs #[cfg(feature="demo_destructibles")]
      math.rs
    pipelines/
      mod.rs
      common.rs        // typed BGL wrappers, common layouts
      terrain.rs
      instanced.rs
      sky.rs
      post/
        mod.rs
        ao.rs
        ssgi.rs
        ssr.rs
        bloom.rs
  ui/
    mod.rs             // dispatch
    perf.rs
    help.rs
    hotbar.rs
```

```
tools/model-viewer/src/
  main.rs              // bootstrap only
  cli.rs
  app.rs
  viewer.rs
  panels.rs
  loader.rs
  utils.rs
```

```
crates/platform_winit/src/
  lib.rs               // thin, re-exports
  app.rs
  input.rs
  picker.rs
  builder_overlay.rs
  replication.rs
  telemetry.rs
```

```
crates/server_core/src/ecs/
  schedule/
    mod.rs             // build_schedule() that composes stages
    stage_input.rs
    stage_ai.rs
    stage_move.rs
    stage_combat.rs
    stage_cleanup.rs
```

```
crates/net_core/src/snapshot/
  mod.rs               // re-exports; version docs
  encode.rs
  actors.rs
  projectiles.rs
  destructibles.rs
  hud.rs
```

```
crates/render_wgpu/src/gfx/demo/
  vox_onepath.rs       #[cfg(feature="demo_vox")] (moved)
```

---

## PR plan & checklists

### PR 1 — Introduce device/attachments/config scaffolding (render_wgpu)

**Files:** `gfx/mod.rs`, `gfx/renderer/{device.rs,attachments.rs,config.rs}`, `gfx/renderer/init.rs`

**Steps**

1. Create `renderer/device.rs`:

   ```rust
   use std::sync::Arc;
   pub struct GpuCtx {
       pub device: Arc<wgpu::Device>,
       pub queue: wgpu::Queue,
       pub adapter: wgpu::Adapter,
       pub limits: wgpu::Limits,
       pub features: wgpu::Features,
   }
   pub struct SurfaceCtx {
       pub surface: wgpu::Surface,
       pub config: wgpu::SurfaceConfiguration,
       pub size: winit::dpi::PhysicalSize<u32>,
   }
   pub struct Samplers {
       pub linear: wgpu::Sampler,
       pub nearest: wgpu::Sampler,
       // add others as fields (moved only)
   }
   impl GpuCtx { /* new() and helper constructors moved from init.rs (no logic changes) */ }
   ```
2. Create `renderer/attachments.rs` with the existing attachments fields moved verbatim into an `Attachments` struct and a `recreate_for_size()` method (logic copied as-is).
3. Create `renderer/config.rs` and move constants/toggles from `mod.rs`/`init.rs` here. Re‑export them back from `gfx::mod.rs` to preserve imports.
4. In `renderer/init.rs`, replace direct device/surface creation with calls to `GpuCtx::new()` and `SurfaceCtx` helpers (copy exact logic).
5. In `gfx/mod.rs`:

   * Replace raw `device`, `queue`, `surface` fields on `Renderer` with `gpu: GpuCtx`, `surface: SurfaceCtx`, `samplers: Samplers`, `attachments: Attachments`.
   * Add **compatibility re‑exports**:

     ```rust
     pub use crate::gfx::renderer::device::{GpuCtx, SurfaceCtx, Samplers};
     pub use crate::gfx::renderer::attachments::Attachments;
     pub use crate::gfx::renderer::config::*;
     ```
6. Update all internal references (`self.device` → `self.gpu.device` etc.).
7. Build & run `cargo xtask ci`.

**Acceptance**

* Compiles; CI green.
* No function signatures changed outside `gfx` crate public surface OR they are aliased via `pub use`.
* Window resize still works (manual smoke test).

---

### PR 2 — Stand up pipelines directory with typed wrappers (mechanical move)

**Files:** `gfx/pipeline.rs` → `gfx/renderer/pipelines/*`

**Steps**

1. Create `renderer/pipelines/common.rs`:

   ```rust
   #[derive(Clone)]
   pub struct Bgl<T> { pub raw: wgpu::BindGroupLayout, _phantom: std::marker::PhantomData<T> }
   pub struct Pipeline<T> { pub raw: wgpu::RenderPipeline, _phantom: std::marker::PhantomData<T> }
   // Example typed tags:
   pub enum TerrainPass {}
   pub enum InstancedPass {}
   pub enum SkyPass {}
   // Post subpasses:
   pub enum AoPass {} pub enum SsgiPass {} pub enum SsrPass {} pub enum BloomPass {}
   ```

   Provide `Deref` to underlying wgpu types for drop‑in usage:

   ```rust
   impl<T> std::ops::Deref for Pipeline<T> { type Target = wgpu::RenderPipeline; fn deref(&self) -> &Self::Target { &self.raw } }
   impl<T> std::ops::Deref for Bgl<T> { type Target = wgpu::BindGroupLayout; fn deref(&self) -> &Self::Target { &self.raw } }
   ```
2. For each pipeline currently built in `pipeline.rs`, create a module (`terrain.rs`, `instanced.rs`, `sky.rs`, `post/{ao,ssgi,ssr,bloom}.rs`) and **move the pipeline creation functions** unchanged, but make them return `Pipeline<Tag>` and any BGLs as `Bgl<Tag>`.
3. Add `renderer/pipelines/mod.rs` that re‑exports all builders and provides a `Pipelines` struct grouping them.
4. In old `gfx/pipeline.rs`, **do not delete**; replace contents with:

   ```rust
   #[deprecated(note="moved to renderer::pipelines")]
   pub use crate::gfx::renderer::pipelines::*;
   ```
5. Update call sites to use `Pipelines` if trivial; otherwise keep using old names via re‑export for now.

**Acceptance**

* All pipelines build exactly once per init with the same configuration.
* No rendering differences (smoke test a scene).
* Clippy clean.

---

### PR 3 — Framegraph skeleton (forwarder only)

**Files:** `gfx/renderer/render.rs`, `gfx/renderer/graph.rs`

**Steps**

1. Add `renderer/graph.rs` with:

   ```rust
   pub struct FrameGraph;
   impl FrameGraph {
       pub fn run(renderer: &mut crate::gfx::Renderer, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
           // TEMP: directly call the existing render code (no reordering)
           super::render::record_frame(renderer, encoder, view);
       }
   }
   ```
2. In `render.rs`, rename the top‑level per‑frame function to `record_frame` (unchanged body).
3. Call `FrameGraph::run` from the place where the encoder is created.
4. Add TODO markers identifying natural pass boundaries (sky, main, post, present), **no behavior change**.

**Acceptance**

* Full render still works; code compiles.
* Graph skeleton exists for later pass extraction.

---

### PR 4 — Split renderer::update by concern

**Files:** `gfx/renderer/update.rs` → `gfx/renderer/update/*`

**Steps**

1. Create `update/{projectiles.rs,builder.rs,math.rs,destructibles_demo.rs}` and move functions intact.
2. In `update/mod.rs`, re‑export moved functions so external call sites stay identical:

   ```rust
   pub mod projectiles; pub mod builder; pub mod math;
   #[cfg(feature="demo_destructibles")] pub mod destructibles_demo;
   pub use projectiles::*; pub use builder::*; pub use math::*;
   #[cfg(feature="demo_destructibles")] pub use destructibles_demo::*;
   ```
3. If any destructible/demo code wasn’t gated, add `#[cfg(feature="demo_destructibles")]` (feature defined in `Cargo.toml`).
4. Add **unit tests** for pure math helpers in `math.rs`.

**Acceptance**

* All previous `use` sites compile without changes.
* Demo code excluded from default build.

---

### PR 5 — Split UI into focused modules

**Files:** `gfx/ui.rs` → `gfx/ui/*`

**Steps**

1. Create `ui/{mod.rs,perf.rs,help.rs,hotbar.rs}`. Move batching/building functions into their files.
2. Keep render‑facing entry points in `ui::mod` and re‑export old names.
3. Add one CPU‑only test asserting expected vertex count for a tiny HUD layout (no GPU needed).

**Acceptance**

* HUD renders as before (manual check).
* At least one unit test for vertex builder.

---

### PR 6 — tools/model-viewer split (bootstrap main)

**Files:** `tools/model-viewer/src/*`

**Steps**

1. Extract `cli.rs` (clap), `loader.rs` (GLTF IO), `viewer.rs` (render loop), `panels.rs` (UI), `app.rs` (state), `utils.rs` (logging/snapshot helpers). Move code verbatim.
2. Keep `main.rs` to:

   ```rust
   fn main() -> anyhow::Result<()> {
       let args = cli::Args::parse();
       let mut app = app::App::new(args)?;
       app.run()
   }
   ```
3. Add a small unit test for `loader.rs` (e.g., bounds rebasing decision) using a synthetic/minimal scene.

**Acceptance**

* Tool runs with identical CLI; help text unchanged.
* New modules compile; tests pass.

---

### PR 7 — platform_winit split

**Files:** `crates/platform_winit/src/*`

**Steps**

1. Move event loop logic to `app.rs` (struct `ApplicationHandler`), input mapping to `input.rs` (keybinds → intents), zone picker to `picker.rs`, builder overlay to `builder_overlay.rs`, replication wiring to `replication.rs`, telemetry init to `telemetry.rs`.
2. Keep `lib.rs` as re‑exports and minimal glue.
3. Add unit test(s) for input mapping (pure function from key sequence → intent).

**Acceptance**

* App launches as before; hotkeys unchanged.

---

### PR 8 — server_core schedule staged

**Files:** `crates/server_core/src/ecs/schedule.rs` → `ecs/schedule/*`

**Steps**

1. Create stage files and move system registration blocks verbatim.
2. `schedule::mod.rs`:

   ```rust
   pub fn build_schedule() -> Schedule {
       let mut s = Schedule::new();
       stage_input::add(systems(&mut s));
       stage_ai::add(&mut s);
       // ...
       s
   }
   pub fn system_names_for_test() -> Vec<&'static str> { /* iterate schedule graph, collect names */ }
   ```
3. Add a test that asserts a few critical orderings by name (string compare only).

**Acceptance**

* System order unchanged (verified by test).
* `build_schedule()` callers unchanged.

---

### PR 9 — server_core lib.rs trims

**Files:** `crates/server_core/src/lib.rs` and moved inline mods

**Steps**

1. Move any inline module bodies into `src/<mod>.rs`.
2. Reduce `lib.rs` to `#![doc = include_str!("../README.md")]` (if applicable), `pub mod` declarations, and re‑exports.

**Acceptance**

* Public API surface unchanged (compile downstream crates).

---

### PR 10 — net_core snapshot split

**Files:** `crates/net_core/src/snapshot.rs` → `snapshot/*`

**Steps**

1. Move message structs into domain files; move common encode/decode helpers into `encode.rs`.
2. Keep wire version doc comments at the top of each domain file.
3. `snapshot/mod.rs` re‑exports the same types names as before.

**Acceptance**

* All serialization tests keep passing.
* No wire format changes.

---

### PR 11 — Move demo vox path behind feature & out of core

**Files:** `crates/render_wgpu/src/gfx/vox_onepath.rs` → `gfx/demo/vox_onepath.rs`

**Steps**

1. Create feature `demo_vox` in `render_wgpu/Cargo.toml`. Wrap module with:

   ```rust
   #[cfg(feature="demo_vox")]
   pub mod demo { pub mod vox_onepath; }
   ```
2. Replace old imports with gated paths or re‑export under `gfx::demo` for parity.
3. Add a CI guard in your pipeline (or `xtask`) to confirm `demo_vox` is **not** enabled by default.

**Acceptance**

* Default build excludes the demo; behavior unchanged when feature enabled.

---

## Mechanical refactor recipe (repeatable)

* **Find usages:** `rg -n "Renderer {"`, `rg -n "create_pipeline"` to plan moves.
* **Move with git:** `git mv` to preserve history.
* **Alias old names:** after a move, add `pub use super::new_path::*;` in the old module immediately.
* **Type wrappers:** prefer `newtype` with `Deref` to avoid churn; keep field names identical.
* **Feature gates:** prefer positive `#[cfg(feature="foo")]` and provide `#[cfg(not(feature="foo"))]` stub functions returning `unimplemented!()` if needed during transition (temporary).
* **Docs:** add `///` rustdoc for every new public type. Keep it 1–2 lines.

---

## Minimal code templates you’ll need

**Typed BGL & Pipeline wrappers**

```rust
pub struct Bgl<T> { pub raw: wgpu::BindGroupLayout, _t: std::marker::PhantomData<T> }
pub struct Pipeline<T> { pub raw: wgpu::RenderPipeline, _t: std::marker::PhantomData<T> }
impl<T> std::ops::Deref for Bgl<T> { type Target = wgpu::BindGroupLayout; fn deref(&self) -> &Self::Target { &self.raw } }
impl<T> std::ops::Deref for Pipeline<T> { type Target = wgpu::RenderPipeline; fn deref(&self) -> &Self::Target { &self.raw } }
```

**Attachments**

```rust
pub struct Attachments {
    pub color: wgpu::TextureView,
    pub depth: wgpu::TextureView,
    pub msaa: Option<wgpu::TextureView>,
}
impl Attachments {
    pub fn recreate_for_size(&mut self, gpu: &GpuCtx, surface: &SurfaceCtx) {
        // copy existing logic here (no behavior change)
    }
}
```

**Framegraph skeleton**

```rust
pub enum PassId { Sky, Main, Post, Present }
pub struct FrameGraph;
impl FrameGraph {
    pub fn run(renderer: &mut Renderer, encoder: &mut wgpu::CommandEncoder, backbuffer: &wgpu::TextureView) {
        super::render::record_frame(renderer, encoder, backbuffer);
    }
}
```

---

## Validation & testing

* **CI:** `cargo xtask ci` (fmt, clippy `-D warnings`, WGSL validation, tests, schema checks).
* **Unit tests to add (CPU‑only):**

  * `ui` vertex builder returns expected counts.
  * `update::math` helpers.
  * `server_core::ecs::schedule::system_names_for_test` preserves key orderings.
  * `net_core::snapshot::encode` round‑trip for a tiny struct.
* **Manual smoke:** run a scene, resize window, toggle UI, and run model‑viewer.

---

## Risk controls & rollback

* Keep **re‑exports** until the end of phase two; only delete shims after downstream crates have been updated.
* Land PRs in the order above to minimize churn.
* If a PR causes visual diffs, roll back and re‑split into smaller moves (most issues come from missed field renames like `device` → `gpu.device`).

---

## Deliverables checklist (phase one)

* [ ] `GpuCtx`, `SurfaceCtx`, `Samplers`, `Attachments`, `renderer::config` in place and used by `Renderer`.
* [ ] `renderer::pipelines/*` exists; old `gfx/pipeline.rs` re‑exports new API.
* [ ] `renderer::graph` forwards to existing render function.
* [ ] `renderer::update/*` split; demo code feature‑gated.
* [ ] `gfx/ui/*` split; one unit test added.
* [ ] `tools/model-viewer` split; `main.rs` bootstrap only.
* [ ] `platform_winit` split; input mapping test added.
* [ ] `server_core::ecs::schedule/*` split; order test added.
* [ ] `server_core::lib.rs` slimmed to re‑exports/docs.
* [ ] `net_core::snapshot/*` split; encode helpers isolated; tests passing.
* [ ] `vox_onepath` moved under feature and out of core path; default build excludes it.

---

### Notes for the agent

* When in doubt about module boundaries, **prefer moving code verbatim** and re‑exporting, rather than “improving” signatures in phase one.
* Add `#[allow(dead_code)]` **temporarily** only if needed to land a move; remove it in the next PR where usage is wired up.
* Keep commit messages action‑oriented (e.g., `render_wgpu: extract GpuCtx/SurfaceCtx; no logic change`).

This completes phase one: establish boundaries, typed shells, and feature hygiene—without touching runtime behavior. Phase two can then safely tackle pass extraction into the framegraph, draw‑list grouping, and targeted cleanups with meaningful tests.
