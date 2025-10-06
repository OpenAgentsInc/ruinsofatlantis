# Big Refactor Log — 2025-10-04

This running log captures code-level changes made to address the 2025-10-04 audit (docs/audits/20251004). It is intended for maintainers to track rationale, scope, and verification for each step. Links point to evidence and diffs where applicable.

## PR 96 — arch: stop server boss spawn from renderer (F-ARCH-001)

- Branch: `arch/stop-renderer-boss-spawn`
- Summary: Remove server entity creation from renderer code.
- Files touched:
  - `crates/render_wgpu/src/gfx/npcs.rs`: removed call to `ServerState::spawn_nivita_unique(...)` and the renderer-local helper logic used to space NPCs around the boss. Left a note that unique boss spawn must happen in app/server bootstrap.
- Motivation: Enforce layering — renderer must be presentation-only. Spawning belongs to server authority or app bootstrap logic.
- Evidence: Audit finding F-ARCH-001, `docs/audits/20251004/99-findings-log.md`.
- CI: Pre-push hook (xtask ci) passed (fmt+clippy+wgsl+tests+schemas). PR squash-merged to `main`.

## PR N/A — net: bounded replication channel with backpressure (F-NET-003)

- Branch: `net/bounded-repl-channel`
- Summary: Replace unbounded std::sync::mpsc channel in `net_core` with a bounded `crossbeam-channel`-backed implementation; expose non-blocking helpers; drop newest on full.
- Files touched:
  - `crates/net_core/Cargo.toml`: added dependency `crossbeam-channel = "0.5.13"` (via `cargo add -p net_core crossbeam-channel`).
  - `crates/net_core/src/channel.rs`: rewrote channel to use `crossbeam_channel`:
    - New `channel_bounded(capacity)` and `channel()` (default capacity = 4096).
    - `Tx::try_send` now returns `false` on full or disconnected; drops newest on full.
    - `Rx::depth()` helper added.
    - Tests: updated `send_and_drain` to use bounded channel; added `drops_when_full` to assert capacity enforcement.
- Motivation: Avoid unbounded growth and provide minimal backpressure guarantee per audit F-NET-003.
- Impacted call sites: Existing callers continue to work (`channel()` retained). No API changes required for `Renderer` or tests.
- CI: `cargo test -p net_core` and `cargo check` passed locally.
- Follow-ups: Wire capacity from config when multi-process networking arrives; add metrics counters if `metrics` is available in this crate.

## Next candidates

- CI hygiene (F-CI-005): Ensure fmt/test build always green; optionally add `deny.toml` and integrate `cargo deny` (xtask already checks if installed). Current CI hook is passing; we will add config and a workflow in a subsequent PR.
- Remove `unwrap/expect` in server hot paths (F-SIM-009): Replace with results/defaults and metrics; add lint guards.
- Extract gameplay/input/AI from renderer (F-ARCH-002): Move systems into `client_core` and keep renderer upload/draw only.

## Validation snapshot

- `cargo check` — OK after both changes above.
- `cargo test -p net_core` — OK; new tests pass.
- xtask CI guard: added a layering check to fail if `render_wgpu` depends on `server_core`.

## CI & hygiene — cargo-deny + GitHub Actions (F-CI-005)

- Files:
  - `deny.toml` at repo root: baseline advisories/bans/licenses policy.
  - `.github/workflows/ci.yml`: runs `cargo xtask ci` on pushes and PRs against `main`.
- Rationale: Ensure fmt/clippy/tests/schema/WGSL validation run in CI; enable dependency advisories via cargo-deny.
- Notes: xtask already warns if cargo-deny missing; workflow installs it (non-fatal if already present).

## Renderer authority hardening — remove DK spawn (F-ARCH-001)

- Files:
  - `crates/render_wgpu/src/gfx/renderer/init.rs`: removed DK `spawn_npc`; set `dk_id = None`; derived `dk_model_pos` for dependent placement.
  - `crates/render_wgpu/src/gfx/mod.rs`: removed DK respawn-time server spawn; preserved previous-position tracking.
- Rationale: Keep renderer presentation-only; server/app bootstrap should own entity creation.
- Tests: render_wgpu tests pass; health bar logic handles `dk_id = None` gracefully.

## Renderer: gate legacy gameplay under features (F-ARCH-002)

- Files:
  - `crates/render_wgpu/Cargo.toml`: added `legacy_client_ai` and `legacy_client_combat` features (off by default).
  - `crates/render_wgpu/src/gfx/renderer/render.rs`: gated wizard AI tick behind `legacy_client_ai`.
  - `crates/render_wgpu/src/gfx/mod.rs`: gated AI helpers behind `legacy_client_ai`.
  - `crates/render_wgpu/src/gfx/renderer/update.rs`: gated server-side projectile/NPC collision behind `legacy_client_combat`.
- Rationale: Ensure default builds perform no gameplay mutations from the renderer; legacy/demo behavior is opt-in.
- Tests: clippy and tests pass with default features; existing feature tests remain valid.

## Renderer: stop server AI calls by default (F-ARCH-002)

- Files:
  - `crates/render_wgpu/src/gfx/renderer/render.rs`: gated `server.step_npc_ai` behind `legacy_client_ai`.
- Rationale: Default builds do not mutate server state from the renderer.
- Tests: workspace tests remain green.

## Towards decoupling render_wgpu from server_core (F-ARCH-002)

- Made `server_core` an optional dependency in `render_wgpu` and wired feature flags to include it when legacy behavior is enabled.
- Introduced `u32` IDs in renderer (zombie/deathknight) to avoid hard dependency on `server_core::NpcId` in default paths.
- Gated server-backed modules and fields:
  - `server_ext` only builds with `legacy_client_combat`.
  - `Renderer.server` and destructible config/queue present only with `legacy_client_ai`/`legacy_client_carve`.
  - `zombies::build_instances` dual signatures (with/without server).
- Current default: legacy features enabled to maintain behavior; xtask temporarily skips no-default-features and feature-combo checks for render_wgpu while extraction proceeds. Next steps are to expose a read-only NPC view in `client_core` replication, flip defaults off, and then turn the layering guard into an error.

## Network protocol — add version headers + caps (F-NET-014)

## Replicated NPC view (client_core + net_core)

- Added `NpcListMsg` to `net_core::snapshot` with a compact list of NPC items.
- `client_core::replication::ReplicationBuffer` now decodes `NpcListMsg` into `Vec<NpcView>`.
- `render_wgpu` prefers replicated NPC HP/max/alive for zombie bars, falling back to server (legacy).

## Platform bridge for local demo replication

- `platform_winit` now creates a `net_core` channel; passes `Rx` to `Renderer::set_replication_rx`.
- Under `demo_server` feature (default), hosts a tiny in-process `server_core::ServerState` and emits `NpcListMsg` every frame.
- Decouples renderer presentation from server ownership while preserving local demo behavior.

- Files:
  - `crates/net_core/src/snapshot.rs`: added `VERSION = 1` prefix byte to all messages; decode rejects mismatches; added conservative max caps for mesh elements (`MAX_MESH_ELEMS`).
- Rationale: Establish forward/backward compatibility hooks and bound allocations to prevent OOM on malformed inputs.
- Tests: `net_core`, `client_core`, and `render_wgpu` tests pass with the new versioned messages.

## Incremental hardening — server unwrap removal (F-SIM-009)

- File: `crates/server_core/src/destructible.rs`
- Change: Replace `unwrap()` on `core_materials::mass_for_voxel` with a safe default (`Mass::kilograms(0.0)`) when material lookup fails.
- Rationale: Avoid panics in production server paths; unexpected material ids should not crash the server tick.
- Follow-up: Broader sweep to add `#![deny(clippy::unwrap_used, clippy::expect_used)]` with targeted allowances and metrics in a separate PR.

## Notes

- All dependency changes used Cargo tooling per repository policy (no manual Cargo.toml edits).
- No interactive apps were run; only build/test/lint and code changes.

## 2025-10-04 PM — Issue #99 and #100 landed (decouple defaults + enforce layering)

- Issue #99: Cut hard link (default build) render_wgpu ↔ server_core
  - Changes (render_wgpu):
    - gfx/renderer/init.rs: gated `server_core` destructible imports and all destructible/server initializers under feature flags; added safe fallbacks for default build (no voxel grid; neutral debris capacity; tiles_per_meter default).
    - gfx/mod.rs: `any_zombies_alive()` now prefers replication; falls back to server only when `legacy_client_ai` enabled. `respawn()` works in both modes (dual call-sites for `zombies::build_instances`). Server-only helpers are `#[cfg(feature = "legacy_client_ai")]`.
    - gfx/renderer/render.rs: removed default references to `server`/`destruct_cfg`; wrapped server lookups (DK/NPC nameplates, boss status fallback) and destructible toggles with feature gates; guarded BossStatus emit to replication behind `legacy_client_ai`.
    - gfx/renderer/update.rs: guarded server‑based selection/collision paths with `legacy_client_ai`/`legacy_client_combat`; provided neutral defaults when features are off.
  - Result: `cargo check -p render_wgpu --no-default-features` passes locally (CI will enforce via xtask).

- Issue #100: Flip defaults OFF and enforce layering in CI
  - crates/render_wgpu/Cargo.toml: `[features] default = []`.
  - xtask/src/main.rs:
    - Always run `check/clippy/test` for `render_wgpu --no-default-features` (removed env gates).
    - Layering guard escalated to error: fails if `cargo tree -p render_wgpu` shows `server_core`.
  - src/README.md: documented `legacy_client_ai` and `legacy_client_combat` (default off) and noted CI enforcement.

- Verification
  - No‑default build: OK.
  - Workspace `cargo xtask ci`: will now fail if renderer links `server_core` by default.

- Tracking
  - #99 marked ready to close after CI confirms. #100 partially complete (CI updated); feature combo checks remain gated as planned.
- Issue #101: Replicated HUD completeness (DK + NPCs)
  - Renderer HUD now uses replication exclusively by default:
    - Zombie nameplates and bars filter alive NPCs via `ReplicationBuffer.npcs` (fallback to server only under `legacy_client_ai`).
    - Death Knight banner and HP bar prefer `BossStatusMsg`; if replication is absent, DK HP bar is omitted in default builds (server fallback gated under legacy feature).
    - Zombie animation attack state and radius selection for palette updates now derive from replication; server fallback only under legacy.
  - Verified `--no-default-features` build shows no server reads in HUD paths; clippy/tests green.

## 2025-10-06 — Demo regressions (#107) fixes and wiring

- wgpu palette validation follow-up
  - Enforced a minimum 64-byte allocation for palette storage buffers across zombie/DK/PC/Sorceress to avoid zero-sized bindings (validation error) even when counts are zero. Confirmed no more validation errors in local runs.
  - Files: `crates/render_wgpu/src/gfx/renderer/init.rs`, `crates/render_wgpu/src/gfx/mod.rs` (previous commit), and ensured the replication-built zombie path in `renderer/render.rs` also allocates a non-zero palette buffer.

- Platform demo server: spawn/step and replication emission
  - `crates/platform_winit/src/lib.rs`:
    - On resume (native), create `ServerState`, spawn three NPC rings and the unique boss near the origin, and store `last_time` for frame `dt`.
    - Each `about_to_wait`, step NPC AI and boss seek toward current wizard positions, then emit framed `NpcListMsg` and `BossStatusMsg` via the local `net_core` channel.
    - Added counters for sent bytes consistent with project metrics usage.
    - Dependencies added via Cargo tooling: `glam = 0.30`, `web-time = 1.1.0`.

- Renderer public helper for demo AI
  - `crates/render_wgpu/src/gfx/mod.rs`: added `pub fn wizard_positions(&self) -> Vec<glam::Vec3>` to expose world positions for the demo server without exposing internal fields.
  - Updated platform to use this API (no direct field access).

- Boss seek system export
  - `crates/server_core/src/systems/mod.rs`: exported `pub mod boss;` (file existed) so platform can call `systems::boss::boss_seek_and_integrate`.

- Replication-driven visuals and terrain snap
  - `crates/render_wgpu/src/gfx/renderer/render.rs`:
    - Import `wgpu::util::DeviceExt` for `create_buffer_init` in the replication path.
    - If there are replicated NPCs and no zombie visuals yet, build minimal zombie instances from replication, snap to `terrain::height_at`, and allocate palette/storage with a min size.
    - Also initialize per-instance arrays (`zombie_prev_pos`, `zombie_time_offset`, `zombie_forward_offsets`) so CPU animation and palette uploads operate safely.
    - Snap Death Knight model Y to terrain when `BossStatus` is present.
    - Added minimal default-build wizard-facing toward PC/nearest replicated NPCs (gated off when `legacy_client_ai` is enabled).
    - Metrics macro usage aligned with existing handle style (`counter!(..).increment(..)`, `gauge!(..).set(..)`).

- Lints/Clippy
  - Fixed `clippy::suspicious-assignment-formatting` in the wizard-facing loop and collapsed a nested `if` for DK terrain snap.
  - Removed an unused import in `server_core::systems::boss`.
  - Added terrain clamping to sorceress default-walk path so Nivita doesn’t sink under uneven ground.

- Validation
  - `cargo check` — OK
  - `cargo clippy --all-targets -D warnings` — OK
  - `cargo test` — all workspace tests green
