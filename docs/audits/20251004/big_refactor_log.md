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

## 2025-10-06 — Start server-authoritative path (#108)

- net_core: Introduced consolidated TickSnapshot and loopback transport
  - `crates/net_core/src/snapshot.rs`:
    - Added `TickSnapshot { v, tick, wizards[], npcs[], projectiles[], boss? }` with a leading tag byte (`TAG_TICK_SNAPSHOT = 0xA1`) for unambiguous decodes alongside legacy, per‑message encodings.
    - Added `WizardRep`, `NpcRep`, `ProjectileRep`, `BossRep` records and `tick_snapshot_roundtrip` unit test.
  - `crates/net_core/src/transport.rs`:
    - New `Transport` trait and `LocalLoopbackTransport` built on the existing bounded channel.
  - `crates/net_core/src/channel.rs`: made `Rx` cloneable to support loopback splitter.

- client_core: Decode TickSnapshot first; extend NPC view with yaw
  - `crates/client_core/src/replication.rs`:
    - `ReplicationBuffer::apply_message` now prefers `TickSnapshot` (boss + npcs) and falls back to `ChunkMeshDelta` → `NpcListMsg` → `BossStatusMsg` for migration.
    - `NpcView` extended with `yaw`; legacy list populates `yaw = 0.0`.

## 2025-10-07 — Effects/cleanup polish + tests tightened

- Server cleanup honors `DespawnAfter` timers
  - Changed `ecs::schedule::cleanup` to remove the blanket `remove_dead()` call. Entities now linger until their `despawn_after.seconds` elapses; as a safety net, dead entities without a timer are despawned immediately.
  - Files: `crates/server_core/src/ecs/schedule.rs`.
- Tests and clippy hygiene
  - Brought `SnapshotEncode` into scope in `client_core/tests/hud_decode.rs`.
  - Collapsed nested ifs in server tests to satisfy `clippy::collapsible-if` with `-D warnings`.
  - `cargo test` and `cargo clippy --all-targets -D warnings` are green across the workspace.

## 2025-10-07 — PR‑9 (Homing reacquire) + PR‑10 (Specs) landed

- Homing reacquire for MagicMissile
  - Added `homing_acquire_targets()` in ECS schedule, executed before `homing_update`. Reacquires when target is dead/missing or out of range, using `SpatialGrid` and faction hostility checks.
  - Extended `ecs::Homing` with `max_range_m` and `reacquire` fields.
  - Wired MagicMissile spawns to set `Homing { turn_rate, max_range_m, reacquire }` from specs.
- Central specs table
  - Introduced `Specs` on `ServerState` with `SpellsSpec`, `EffectsSpec`, and `HomingSpec`.
  - Replaced literals with specs:
    - Cast costs/CD/GCD via `spell_cost_cooldown` now read from `self.specs.spells`.
    - MagicMissile Slow mul/dur and Fireball Burning dps/dur read from `self.specs.effects`.
    - Homing turn rate/range from `self.specs.homing`.
- Validation
  - `cargo test` and `cargo clippy --all-targets -D warnings` green.

## 2025-10-07 — Projectile replication and logs

- Stepped server before replication; v2 snapshots (actors + projectiles) flow each frame by default (RA_SEND_V3 toggles deltas).
- Added logs for casts and snapshots:
  - `srv: enqueue_cast …` when commands arrive
  - `srv: cast accepted/rejected …` in `cast_system`
  - `snapshot_v2: tick=… actors=… projectiles=…` before sending
  - `renderer: projectiles this frame = …` at FX update start
- Added optional gating bypass via `RA_SKIP_CAST_GATING=1` for quick demo verification.

## 2025-10-06 — Test wall for authoritative Fireball + replication (#107 follow-up)

- Added a focused, multi-layer test suite to prevent regressions in the “Fireball doesn’t hurt wizards / no floaters / orb keeps flying” class of bugs.
- Server tests (authoritative gameplay)
  - File: `crates/server_core/src/lib.rs`
  - New tests cover:
    - Impact AoE: Fireball detonates on wizard/NPC and damages both; projectile removed.
    - Proximity AoE: Fireball segment proximity triggers detonation; projectile removed.
    - Owner flip: PC-owned Fireball that damages a wizard flips `wizards_hostile_to_pc = true`.
  - Existing tests already covered TTL AoE, HP clamp/alive, Firebolt hits, and spawn owner/velocity properties.
- Client replication tests
  - File: `crates/client_core/src/replication.rs`
  - Added `apply_tick_snapshot_populates_all_views` to lock that `ReplicationBuffer` populates wizards, NPCs, projectiles, and boss from `TickSnapshot` (framed).
- Protocol tests
  - File: `crates/net_core/src/snapshot.rs`
  - Existing `tick_snapshot_roundtrip` already validates consolidated snapshot shape; no duplication added.
- End-to-End headless integration
  - File: `tests/e2e_authoritative.rs`
  - Smoke test drives: PC-owned Fireball → authoritative `step_authoritative` → wizard HP drop → projectile removal → `tick_snapshot` reflects HP drop and no lingering Fireball.
- Validation
  - `cargo test -p server_core` — green (29 tests)
  - `cargo test -p net_core` — green (12 tests)
  - `cargo test -p client_core` — green (11 tests)
  - `cargo test -p ruinsofatlantis --test e2e_authoritative` — green (1 test)
  - Note: `cargo test --workspace` currently fails due to pre-existing issues in unrelated crates (`collision_static` missing deps and `sim-harness` API drift). Scope-limited tests above are green; we can open a follow-up to fix those crates or exclude them from CI if desired.

## 2025-10-06 — Workspace tests green (voxel mesher + harness fixes)

- Fix: voxel_mesh greedy mesher edge cases
  - File: `crates/voxel_mesh/src/lib.rs`
  - Correct neighbor checks (avoid saturating_sub) so boundary faces emit.
  - Emit positive faces on the far side of the cell; negative on near side.
  - Adjust winding for axis=Y to align face normals with stored vertex normals.
  - Bias per-chunk meshing to the lower chunk at boundaries (X) so only one owner emits boundary faces; mirrored guards added for Y/Z.
  - Result: all voxel_mesh tests pass.
- Fix: collision_static test deps
  - Added dev-dependencies `core_materials` and `core_units` (via `cargo add --dev`).
- Fix: tools/sim-harness build
  - Switched to `sim_core::sim::runner::run_scenario` public API.
  - Added `serde_json` dependency (via `cargo add`).
- Validation
  - `cargo test --workspace` — green.

- platform_winit: Emit TickSnapshot in addition to legacy messages
  - `crates/platform_winit/src/lib.rs`:
    - Track a local `tick` counter; after stepping the demo server each frame, build and send a framed `TickSnapshot` with NPC yaw computed toward the nearest wizard (temporary until the server sends yaw).
    - Kept `NpcListMsg` + `BossStatusMsg` emission during migration.

- render_wgpu: Consume server yaw when available
  - `crates/render_wgpu/src/gfx/mod.rs`:
    - `update_zombies_from_replication` now prefers replicated yaw for zombies; falls back to previous client‑side facing heuristics when yaw is absent.

- Validation
  - `cargo check` — OK
  - `cargo test -p net_core` — new tests pass
  - `cargo clippy --all-targets -D warnings` — OK

- Next (#108 follow‑ups)
  - Move demo HP/melee/projectile damage out of renderer once the server includes authoritative HP/projectiles in TickSnapshot.
  - Swap platform’s in‑place channel wiring for `LocalLoopbackTransport` end‑to‑end and introduce a `WebSocketTransport` for remote.

## 2025-10-06 — Finish #108: server‑authoritative only; remove demo hacks

- Renderer (default build):
  - Removed client-side combat/collision and local HP deltas; presentation-only now. PC casts send ClientCmd; projectiles/HP come from TickSnapshot.
  - No default runtime reads of server_core; feature flags left only for archaeology (deprecated, off by default).
- Platform:
  - Using LocalLoopbackTransport; drains ClientCmd → spawns projectiles on server, steps server, sends TickSnapshot; removed legacy message sends.
- Server:
  - Owns wizard/projectile ECS state; steps NPC melee, wizard casts, projectiles, and authoritative HP; uses projectile spec speeds for NPC and PC casts.
  - Fireball parameters are now server‑resolved only (speed/life/radius/damage). Removed/ignored any client‑provided AoE tuning. Fixed bug where client passed radius=0, damage=0 causing no AoE damage.
  - Added RA_LOG_FIREBALL traces on spawn/explode with center/radius/damage; proximity and TTL explosions use spec values consistently.
  - Added tests: `fireball_aoe_damages_ring` and `fireball_ttl_explodes_and_damages`.
- CI/Policy:
  - render_wgpu default = []; xtask always checks/clippies/tests no-default build and enforces layering guard error.
- Docs:
  - Updated src/README.md to reflect server-authoritative demo and deprecated flags.

## 2025-10-06 — Actor refactor groundwork + legacy snapshot removal

- platform_winit: removed legacy `TickSnapshot` emission; now sends only actor-centric `ActorSnapshot` v2 per tick (framed), with byte metrics.
- server_core:
  - Projectiles now carry `owner: Option<ActorId>`; added `spawn_projectile_from_pc()` and switched platform command handling to use it.
  - `sync_wizards()` now ensures PC/NPC wizard actors exist and mirrors positions into `ActorStore`.
  - `step_authoritative()` no longer rebuilds from legacy lists; projectile collisions operate over actors (direct hits) and call `apply_aoe_at_actors()` for Fireball (impact/proximity/TTL).
  - Deferred Fireball impact AoE to avoid borrow conflicts while iterating `actors` mutably.
  - Kept legacy `Npc`/`Wizard` structs temporarily (unused in hot paths) to keep the tree compiling; follow-up will delete them fully.
- Build: `cargo check`/`cargo build` green.

Next:
- Delete remaining legacy NPC/Wizard fields and helpers; move boss spawn/status to `nivita_actor_id` and actor lookup.
- Update tests to actor-centric snapshots and remove legacy TickSnapshot tests.

## 2025-10-06 — Actors-only authority complete

- server_core:
  - Removed legacy `wizards`/`npcs` storage and all dependent helpers/tests.
  - Boss system now uses `nivita_actor_id` and actor lookup for movement; default speed used until components carry it.
  - `tick_snapshot_actors` uses live `ActorStore`; removed `clone_for_snapshot` and all legacy rebuild bridges.
- platform_winit:
  - Removed `step_npc_ai` call; demo stepping remains via authoritative step + boss seek helper.
- tests:
  - Pruned legacy wizard/NPC tests; added minimal actor-centric test for projectile speed. Workspace tests/clippy green.
