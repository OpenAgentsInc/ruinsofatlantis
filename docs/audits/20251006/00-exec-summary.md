# Executive Summary — 2025-10-07 (ECS Refactor Alignment)

Scope
- Full repo scan against the architecture in `docs/issues/ecs_refactor_part_2.md` with focus on: authoritative ECS world, ordered systems/schedule, event-based damage, spatial index, interest-managed replication, and removal of legacy client-side AI/combat.

Status (high level)
- Core server ECS now matches the refactor doc much more closely:
  - `server_core::ecs::WorldEcs` is authoritative; schedule includes cooldown/mana tick, cast pipeline, ingest of pending spawns, AI move/melee, homing, projectile integrate/collide, AoE, faction, apply‑damage, cleanup, and despawn timers.
  - Projectiles moved into ECS with components (`projectile`, `velocity`, `owner`, `homing`). MagicMissile homes with reacquisition via spatial grid.
  - Effects pipeline active: Fireball applies Burning over time; MagicMissile applies Slow; Stun gates move/melee/cast; unit tests cover these.
  - Replication: v2 snapshots and optional v3 deltas with interest and rate limiting; new HUD status channel sends mana, GCD, per‑spell CDs, and active effects.
- Primary deviations remaining are legacy client paths (feature‑gated) and the temporary `sync_wizards` bridge for PC/NPC wizard positions and PC respawn.

Top Deviations
- Legacy scaffolding remains in renderer and client: `legacy_client_ai`, `legacy_client_combat`, and compatibility decoders for `NpcListMsg`/`BossStatusMsg` are still in tree. See `crates/render_wgpu/src/gfx/renderer/update.rs:2035`, `crates/client_core/src/replication.rs:218`.
- `ActorStore` (pre‑ECS) type still exists (unused by `ServerState`). See `crates/server_core/src/actor.rs:58`.
- `ServerState::sync_wizards()` still mirrors renderer wizard positions; it also respawns the PC and attaches casting resources if absent/dead. Long‑term, server movement/respawn should be authoritative via intents and policy. See `crates/server_core/src/lib.rs:216`.
- Spatial grid is present and used for AoE/homing reacquire but is rebuilt each tick; projectile segment broad‑phase still not fully cell‑culled. See `crates/server_core/src/ecs/schedule.rs:120` and `:1080` (grid).

Impact
- With defaults, the app honors server authority (casts, projectiles, AI, effects, death/despawn) and replication; legacy code is off by default. Remaining legacy/bridge code adds maintenance overhead and small coupling risks.

Plan (condensed)
- Phase out legacy client AI/combat/features and delete unused pre‑ECS types.
- Replace `sync_wizards` with authoritative movement intents and a respawn policy (server‑side).
- Incrementalize spatial grid and use for projectile segment broad‑phase.
- Make v3 deltas the default (RA_SEND_V3=1 by default) and remove `NpcListMsg`/`BossStatusMsg` compatibility.
