# Migration Plan — Bring Codebase Fully In‑Line (No Legacy)

Targets are mapped to the refactor doc’s phases, with concrete repo changes.

Phase A — Delete legacy and pre‑ECS scaffolding (low risk)
- Remove `legacy_client_ai`, `legacy_client_combat`, and `legacy_client_carve` features and their code branches from `crates/render_wgpu`.
- Delete `crates/render_wgpu/src/server_ext.rs` and any `pub use server_core` exposure guarded only by legacy features.
- Delete pre‑ECS `ActorStore` and stale comments in `crates/server_core/src/actor.rs` and `crates/server_core/src/lib.rs` (bridge notes).
- Remove compatibility decoders for `NpcListMsg` and `BossStatusMsg` in `crates/client_core/src/replication.rs`; retain only actor snapshots (v2) and deltas (v3).
- Update `src/README.md` and `docs` to state that actor snapshots are canonical; note that legacy messages are gone.

Phase B — Server‑side input intents and authoritative movement
- Add `IntentMove { dir: Vec2, run: bool }` and `IntentAim { yaw: f32 }` components.
- Platform continues draining `ClientCmd`; extend to send movement intents. In schedule, add an InputSystem before AI to apply intents.
- Remove `sync_wizards` position mirroring and move PC respawn into a server policy system (e.g., `RespawnSystem`).
- Derive `yaw` for wizards server‑side (face reticle/target).

Phase C — Spatial grid incrementalization + projectile broad‑phase
- Move `SpatialGrid` into `WorldEcs` and update buckets on actor `Transform` writes (dirty on move).
- Provide queries: `query_circle(center, r)` and `query_segment(p0, p1, pad)` (iterate overlapping cells, return candidate actors).
- Rewrite projectile collision to use grid candidates only; keep proximity explode using grid.

Phase D — Homing missiles and richer components (optional)
- Add `Homing { turn_rate: f32, target: Option<ActorId> }`; update projectile integrate to home toward `Target`.
- Add `Target(ActorId)` component and a simple selection system for PC spells.

Phase E — Observability and guardrails
- Convert server logs to `tracing`; add per‑system timings and counters (actors processed, events emitted, projectiles integrated, grid rebuilds/updates).
- Add unit tests for input system (deterministic movement), projectile collision (grid path), and melee cooldown logic.
- Ensure `cargo xtask ci` validates no legacy features exist and clippy is clean.

Phase F — Finalize replication
- Make v3 deltas default; keep v2 encode only for tooling/backcompat during a short window.
- Delete `NpcListMsg`/`BossStatusMsg` decoders in client; ensure HUD derives only from `HudStatusMsg` and actor snapshots.
- Confirm v3 deltas cover all fields client needs; keep interest management enabled by default.

Outcomes
- No legacy paths in renderer or client; server ECS is the only authority.
- Docs and README reflect the true architecture; easier onboarding and maintenance.
