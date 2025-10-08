Remediation Plan (ordered, with acceptance criteria)

1) Remove archetype branching from systems (P0)
- Actions
  - Replace `wizard_targets()` with a Faction‑based or component predicate (e.g., Faction::Wizards).
  - Remove `ActorKind::Wizard` assignment from projectile spawns (projectiles do not carry ActorKind semantics).
- Files
  - crates/server_core/src/ecs/schedule.rs:97, 147, 184, 658
- Acceptance
  - rg “ActorKind::Wizard” returns 0 in server systems (allow in spawn wiring only if strictly required for replication mapping).
  - All unit/integration tests green.

2) Grid `query_segment` + projectile broad‑phase (P0)
- Actions
  - Implement `SpatialGrid::query_segment(a, b, pad)`.
  - In `projectile_collision_ecs`, collect candidate IDs from grid and only test those.
- Files
  - crates/server_core/src/ecs/schedule.rs (SpatialGrid impl + projectile collision)
- Acceptance
  - New micro test asserting candidate count ≪ total for spread actors.
  - Integration perf smoke unchanged or improved; logic tests green.

3) Extend ActorRep with IDs and remove legacy runtime HUD types (P0)
- Actions
  - Add `archetype_id: u16`, `name_id: u16`, `unique: u8` to ActorRep.
  - Populate from server ECS spawn sites; maintain client mapping in renderer.
  - Remove `BossStatusMsg` and `NpcListMsg` from runtime path (keep tests/doc samples if needed, or delete outright).
- Files
  - crates/net_core/src/snapshot.rs; crates/server_core/src/lib.rs (rep encode); client_core decode/tests.
- Acceptance
  - v3 roundtrip tests updated; no string names on wire; HUD draws solely from v3 + HudStatus.

4) Move literals to Specs/data (P1)
- Actions
  - Add `arming_delay_s` per projectile in data_runtime specs; use in collision.
  - Move default Undead/Wizard/DK move/aggro/melee to data (or Specs archetype table) and read on spawn.
- Files
  - crates/server_core/src/ecs/schedule.rs; crates/server_core/src/lib.rs; crates/data_runtime/**
- Acceptance
  - Grep shows no magic constants for these in systems/spawns.
  - Unit tests validate values loaded from Specs.

5) Normalize HitFx to Ctx event bus (P1)
- Actions
  - Push `HitFx` to `Ctx.hits` in collision; after schedule run, move Ctx.hits into ServerState for platform pickup.
- Files
  - crates/server_core/src/ecs/schedule.rs; crates/server_core/src/lib.rs; crates/platform_winit/src/lib.rs
- Acceptance
  - No direct writes to `ServerState.fx_hits` from systems; tests unchanged.

6) Deny unwrap/expect in server_core (P0)
- Actions
  - Add `#![deny(clippy::unwrap_used, clippy::expect_used)]` in server_core/lib.rs.
  - Replace unwraps with Result flows or clamped defaults; where unavoidable, `expect` with clear message + test coverage.
- Files
  - crates/server_core/**
- Acceptance
  - rg for unwrap/expect (non‑test) returns none; clippy passes.

7) Observability spans & counters (P2)
- Actions
  - Wrap each system in a `tracing::info_span!("system", name=...)`; record counters for events processed.
- Files
  - crates/server_core/src/ecs/schedule.rs
- Acceptance
  - Logs show per‑system span entries under RA_LOG spans build; no perf impact observed.

8) Enforce CI grep guards (P1)
- Actions
  - Add an xtask step or script invoked by pre‑push/CI that fails on `legacy_client_`, `NpcListMsg`, `BossStatusMsg`, `ActorStore` in non‑docs/tests.
- Files
  - xtask/src/main.rs; .githooks/pre-push
- Acceptance
  - Intentional reintroduction of any of the strings causes CI failure.
