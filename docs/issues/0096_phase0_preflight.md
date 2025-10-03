# Phase 0 — Preflight Hygiene and Feature Gates (Standalone Plan)

Labels: refactor, cleanup, tech-debt
Depends on: Epic #95 (ECS/server-authoritative)

Intent
- Safely gate legacy/demo code paths and reduce default logging noise before refactoring gameplay into ECS. This keeps the main runtime stable while we migrate systems server‑side.

Outcomes
- Default build contains no demo grid and performs no client‑side world mutation for destructibles.
- Optional features restore legacy behavior for side‑by‑side testing and debugging.
- CI builds with and without the features to prevent regressions.

---

Repo‑aware Inventory (what to gate/tidy)

Renderer (crates/render_wgpu)
- Demo/one‑path voxel world:
  - `crates/render_wgpu/src/gfx/vox_onepath.rs` (entire module; demo driver)
  - `crates/render_wgpu/src/gfx/renderer/update.rs`:
    - `process_voxel_queues()` (legacy global voxel world queue/mesh path)
    - `build_voxel_grid_for_ruins(..)` (box proxy for ruins; demo)
    - `reset_voxel_and_replay()` (demo helper)
    - Many demo toggles/shortcuts are guarded by `is_vox_onepath()` checks
- Client‑side destructible mutation (to be server‑side):
  - `find_destructible_hit(..)` (selector)
  - `explode_fireball_against_destructible(..)` (entry/dda + carve + enqueue + colliders + debris)
  - `process_one_ruin_vox(..)`, `process_all_ruin_queues(..)` (per‑chunk greedy meshing and uploads)
  - Calls to chunk collider builder under `gfx::chunkcol`
- Verbose destructible logs (info/warn) sprinkled across update.rs/init.rs/render.rs

Server helpers (crates/server_core)
- `crates/server_core/src/destructible.rs` logs are useful; keep defaults quiet and add an opt‑in feature for chatter.

Docs & code references
- `docs/issues/ecs_refactor.md` (master issue list; this is split‑out issue #1)
- `docs/issues/0095_ecs_server_authority_plan.md` (Phase 0 preflight already outlined)

---

Features to add (Cargo)

crates/render_wgpu/Cargo.toml
- `[features]`
  - `legacy_client_carve = []` (default: OFF)
    - Gates client‑side carve/collider/meshing/debris mutation paths
  - `vox_onepath_demo = []` (default: OFF)
    - Gates `vox_onepath.rs`, demo grid creation, and demo helpers in `update.rs`
  - `destruct_debug = []` (default: OFF)
    - Enables verbose info/warn logs for destructible selection/voxelize/colliders/meshing

crates/server_core/Cargo.toml
- `[features]`
  - `destruct_debug = []` (mirror feature to control logs in `destructible.rs`)

Top‑level: no default‑features beyond existing. Keep default runtime quiet and production‑oriented.

---

Code changes (surgical)

1) Gate demo/one‑path module
- Wrap whole `crates/render_wgpu/src/gfx/vox_onepath.rs` with `#![cfg(feature = "vox_onepath_demo")]` and guard its call‑sites with `#[cfg(feature = "vox_onepath_demo")]`.

2) Gate demo voxel world helpers in update.rs
- In `crates/render_wgpu/src/gfx/renderer/update.rs`:
  - Annotate the following functions with `#[cfg(feature = "vox_onepath_demo")]`:
    - `process_voxel_queues()`
    - `build_voxel_grid_for_ruins(..)`
    - `reset_voxel_and_replay()`
  - Where referenced, wrap callers with `#[cfg(feature = "vox_onepath_demo")]` or early returns.

3) Gate client‑side carve/mutation behind legacy flag
- In `crates/render_wgpu/src/gfx/renderer/update.rs`:
  - Annotate `explode_fireball_against_destructible(..)`, `process_one_ruin_vox(..)`, `process_all_ruin_queues(..)` with `#[cfg(feature = "legacy_client_carve")]`.
  - In the projectile loop, guard the branch that calls `explode_fireball_against_destructible(..)` with the same feature; otherwise, skip mutation (will be replaced by replication later).
  - Any direct collider rebuilds (`chunkcol::swap_in_updates`) and debris spawns tied to carves should be wrapped in `legacy_client_carve`.

4) Centralize and gate logs
- Add a tiny helper macro in `update.rs` (and optionally in `server_core::destructible.rs`):
  - `#[cfg(feature = "destruct_debug")] macro_rules! destruct_log { ($lvl:ident, $($t:tt)*) => { log::$lvl!($($t)*) } }`
  - `#[cfg(not(feature = "destruct_debug"))] macro_rules! destruct_log { ($lvl:ident, $($t:tt)*) => { { } } }`
- Replace high‑frequency `log::info!/warn!` destruct logs with `destruct_log!(info, ..)`.
- Keep one‑time startup summaries (e.g., adapter, swapchain) as info.

5) Default runtime behavior audit
- Ensure the following codepaths are NO‑OPs without features:
  - Demo grid is never created; no calls to `build_voxel_grid_for_ruins`.
  - No enqueues to legacy global chunk queues; `process_voxel_queues` doesn’t compile.
  - No client carve/collider/meshing/debris; renderer only renders and uploads.

6) Documentation updates
- Add a brief section to `src/README.md` (or `docs/audits/20251003/00_overview.md`) describing:
  - New features and their purpose
  - How to build with them (`cargo build -p render_wgpu --features vox_onepath_demo,legacy_client_carve,destruct_debug`)

---

Search/replace helpers (to locate call‑sites)

- Demo helpers & queue:
  - `rg -n "process_voxel_queues|vox_onepath|build_voxel_grid_for_ruins|reset_voxel_and_replay" crates/render_wgpu/src/gfx/renderer/update.rs`
- Client carve/mutate:
  - `rg -n "explode_fireball_against_destructible|process_one_ruin_vox|process_all_ruin_queues|chunkcol::" crates/render_wgpu/src/gfx/renderer/update.rs`
- Verbose logs to swap to `destruct_log!`:
  - `rg -n "\\[destruct\\]|voxelize|carve|mesh upload|collider" crates/render_wgpu/src/gfx/renderer`

---

CI additions

- Build matrix (or local xtask) that verifies both:
  - Default features (no demo, no legacy carve)
  - Demo features: `--features vox_onepath_demo,legacy_client_carve,destruct_debug`
- Minimal approach (in `xtask`): add a `ci-extras` step that runs `cargo check -p render_wgpu --features vox_onepath_demo,legacy_client_carve,destruct_debug`.

---

Acceptance Criteria

- Default build:
  - No demo voxel world; no client‑side carve/collider/meshing/debris paths compile.
  - Logs are quiet aside from one‑time setup; destruct logs appear only with `destruct_debug`.
- With features enabled:
  - Behavior matches current legacy paths (demo & client carve) for validation.
- CI:
  - Both default and feature builds compile (and tests run) in CI.

---

Task Checklist

- [ ] Add features to `crates/render_wgpu/Cargo.toml` and `crates/server_core/Cargo.toml`.
- [ ] Guard demo helpers (`vox_onepath` module and related functions) with `vox_onepath_demo`.
- [ ] Guard client carve/collider/meshing/debris code with `legacy_client_carve`.
- [ ] Introduce `destruct_log!` macro and apply to high‑frequency destruct logs.
- [ ] Update docs (`src/README.md` or audit overview) explaining features and use.
- [ ] Update `xtask`/CI to check both default and feature builds.

Notes & Owners
- Renderer: graphics owners to review feature gates and logging impact.
- Server: review `destruct_debug` mirroring.
- QA: validate default vs feature behavior visually and via logs.
