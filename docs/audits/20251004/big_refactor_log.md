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

## Incremental hardening — server unwrap removal (F-SIM-009)

- File: `crates/server_core/src/destructible.rs`
- Change: Replace `unwrap()` on `core_materials::mass_for_voxel` with a safe default (`Mass::kilograms(0.0)`) when material lookup fails.
- Rationale: Avoid panics in production server paths; unexpected material ids should not crash the server tick.
- Follow-up: Broader sweep to add `#![deny(clippy::unwrap_used, clippy::expect_used)]` with targeted allowances and metrics in a separate PR.

## Notes

- All dependency changes used Cargo tooling per repository policy (no manual Cargo.toml edits).
- No interactive apps were run; only build/test/lint and code changes.
