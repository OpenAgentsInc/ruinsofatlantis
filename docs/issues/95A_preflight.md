# 95A — Phase 0: Preflight Hygiene and Feature Gates

Labels: refactor, cleanup, tech-debt
Depends on: Epic #95 (ECS/server-authoritative)

Intent
- Safely gate legacy/demo paths and reduce default logging noise before migrating gameplay into ECS/server authority.

Outcomes
- Default build contains no demo voxel grid and performs no client‑side destructible mutation.
- Optional features restore legacy behavior for side‑by‑side testing.
- CI builds both default and feature combos.

Repo‑aware Inventory
- Renderer (crates/render_wgpu)
  - Demo/one‑path voxel world:
    - `crates/render_wgpu/src/gfx/vox_onepath.rs` (demo driver)
    - `crates/render_wgpu/src/gfx/renderer/update.rs`:
      - `process_voxel_queues()` (legacy global voxel world queue/mesh)
      - `build_voxel_grid_for_ruins(..)` (box proxy)
      - `reset_voxel_and_replay()` (demo helper)
  - Client‑side destructible mutation (to be server‑side):
    - `find_destructible_hit(..)`
    - `explode_fireball_against_destructible(..)`
    - `process_one_ruin_vox(..)`, `process_all_ruin_queues(..)`
    - calls to `gfx::chunkcol` (colliders)
  - Verbose destructible logs in `update.rs`/`init.rs`/`render.rs`
- Server helpers (crates/server_core)
  - `crates/server_core/src/destructible.rs` logging

Features to add (Cargo)
- crates/render_wgpu/Cargo.toml
  - `[features]`
    - `legacy_client_carve = []` (default: OFF) — gate client‑side carve/collider/mesh/debris
    - `vox_onepath_demo = []` (default: OFF) — gate demo module & helpers
    - `destruct_debug = []` (default: OFF) — opt‑in verbose logs
- crates/server_core/Cargo.toml
  - `[features]`
    - `destruct_debug = []` — mirror logging control

Changes (surgical)
1) Gate `vox_onepath` demo and helpers
- Wrap module `vox_onepath.rs` with `#![cfg(feature = "vox_onepath_demo")]` and guard call‑sites.
- Annotate in `renderer/update.rs` with `#[cfg(feature = "vox_onepath_demo")]`:
  - `process_voxel_queues()`, `build_voxel_grid_for_ruins(..)`, `reset_voxel_and_replay()`
2) Gate client carve/collider/mesh/debris behind `legacy_client_carve`
- Annotate in `renderer/update.rs`:
  - `explode_fireball_against_destructible(..)`, `process_one_ruin_vox(..)`, `process_all_ruin_queues(..)`
  - Wrap projectile branch that invokes carve with `#[cfg(feature = "legacy_client_carve")]`.
  - Wrap direct collider rebuilds/debris spawns tied to carve.
3) Centralize/gate logs
- Add `destruct_log!` macro in `renderer/update.rs` (and optionally `server_core::destructible.rs`) gated by `destruct_debug`.
- Swap high‑frequency `info!/warn!` destruct logs to `destruct_log!`.
4) Default runtime audit
- Verify no demo grid is built; legacy queue code does not compile without features; no client mutation occurs.
5) Docs
- Add a section documenting features and usage (build flags) in `src/README.md` or `docs/audits/20251003/00_overview.md`.

CI additions
- Build both:
  - default (no features)
  - `--features vox_onepath_demo,legacy_client_carve,destruct_debug`
- Add an xtask subcommand or matrix job to `ci`.

Acceptance
- Default build: no demo voxel world; no client‑side carve/collider/mesh/debris compiled; logs quiet.
- Feature build: legacy behavior restored.
- CI green for both configurations.
