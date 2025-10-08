2025-10-07 — ECS Refactor Master Log (PR‑0 … PR‑5)

PR‑0 — Faction and v4 replication (landed)
- Renamed Team → Faction across server ECS and docs. Actor has `faction: Faction`.
- ActorSnapshotDelta v4 (net_core): ActorRep includes `{ faction, archetype_id, name_id, unique }`. Platform sends v4; client decodes and stores.
- Server encodes v4; client stores fields in ActorView. Kept `kind` only for presentation bucketing.
- Tests updated to v4 and faction usage.

PR‑1 — Remove archetype branching from systems (landed)
- Deleted `ActorKind`‑based logic in systems; use `faction` and component predicates.
- Projectiles are no longer tagged with a “Wizard” kind. Caster AI selects targets by faction/hostility.
- Tests: caster selection independent of `kind`; PC→Wizard damage flips faction flag.

PR‑2 — Projectile broad‑phase via SpatialGrid (landed)
- Added `SpatialGrid::query_segment(a,b,pad)` and used it in `projectile_collision_ecs`.
- Precise segment‑vs‑circle check unchanged; owner‑skip retained.
- Test asserts candidate set ≪ total actors for a typical scene.

PR‑3 — No‑panic guarantee (landed)
- Enforced `#![deny(clippy::unwrap_used, clippy::expect_used)]` in server_core. Added `#[allow]` only in test modules where unwraps are acceptable.
- Verified non‑test code has no unwrap/expect.

PR‑4 — Data‑driven literals (landed)
- Moved projectile arming delays to data_runtime/specs/projectiles (new `arming_delay_s`). Collision uses spec value.
- Introduced spawn archetype specs (data_runtime/specs/archetypes) for Undead, WizardNPC, DeathKnight. Spawns now read defaults from the spec db; legacy numbers remain as fallback.
- Added tests for archetype defaults.

PR‑5 — Normalize HitFx through Ctx (landed)
- Systems push per‑tick visual hit events to `Ctx.fx_hits`. After the schedule run, `ServerState::step_authoritative` drains `ctx.fx_hits` into `self.fx_hits`.
- Platform continues to read `srv.fx_hits` and replicate in v4 deltas (unchanged network contract).
- Test `hitfx_ctx_bus.rs` ensures server‑auth HitFx flows through Ctx and accumulates in ServerState.

Notes
- Client remains presentation‑only; no gameplay logic. HUD and VFX are driven via replication (v4 + HitFx).
- All changes landed with tests and green CI.

