# ECS Architecture vs Refactor Doc — 2025-10-07

Source of truth: docs/issues/ecs_refactor_part_2.md

What the doc specifies vs what’s implemented now

1) Replace ActorStore with ECS world
- Doc: Use `WorldEcs { world: ecs_core::World, net_map }`; keep public shape stable.
- Code: `server_core::ecs::WorldEcs` (custom, minimal ECS specialized for actors) replaces vec store inside `ServerState` (crates/server_core/src/lib.rs:104,127). It holds `Components { id, kind, team, tr, hp, … }` and offers `spawn/get/iter` (crates/server_core/src/ecs/mod.rs:33-41, 59-68).
- Deviation: `ActorStore` type still exists (not used by `ServerState`) (crates/server_core/src/actor.rs:58). WorldEcs does not wrap `ecs_core::World` as the doc suggested; it’s an internal specialized store.

2) System schedule with events
- Doc: Ordered systems (Input, AISelectTarget, AIMove, Melee, ProjectileIntegrate, ProjectileCollision, AoE, Faction, ApplyDamage, Cleanup, Snapshot).
- Code: Schedule now includes cooldown/mana tick, cast pipeline, projectile spawn ingestion, effects, AI move, melee, homing acquire/update, projectile integrate/collide, AoE, faction, apply‑damage, cleanup with despawn timers (crates/server_core/src/ecs/schedule.rs:60-83). Damage/explosions/deaths flow via events (`DamageEvent`, `ExplodeEvent`, `DeathEvent`).
- Deviation: Player movement is still mirrored via `sync_wizards` rather than intents; casting is authoritative via queued `CastCmd` consumed in schedule.

3) Components for data (not constants)
- Doc: MoveSpeed, Melee, AggroRadius, AttackRadius (and optional Homing).
- Code: Present and used. Homing is implemented for MagicMissile with reacquire (crates/server_core/src/ecs/world.rs:220-252; schedule.rs:820-880). Casting resources (Spellbook, ResourcePool, Cooldowns) added to actors.
- Deviation: None for MVP; consider a `Target` component later for explicit PC targeting.

4) Spatial index
- Doc: 2D XZ uniform grid; use for proximity/collision/AoE.
- Code: `SpatialGrid` present and rebuilt each tick; used for AoE and homing reacquire. Projectile segment collision still scans all actors; next step is segment→cells pruning (crates/server_core/src/ecs/schedule.rs:1040-1180).
- Deviation: Incremental updates and segment cell‑based broad‑phase still pending.

5) Replication: actor snapshots, deltas with interest, HUD
- Doc: ActorSnapshot v2, per‑client interest, and v3 deltas with baseline.
- Code: v2 full snapshots and opt‑in v3 deltas (env `RA_SEND_V3`) with interest radius; new `HudStatusMsg` encodes mana/GCD/spell CDs/effects, platform sends per‑tick, client stores `HudState` (platform: crates/platform_winit/src/lib.rs:300-540, 516-540; net_core: crates/net_core/src/snapshot.rs:560-740; client_core: crates/client_core/src/replication.rs:311-329).
- Deviation: Client still includes compatibility decoders for `NpcListMsg`/`BossStatusMsg` (crates/client_core/src/replication.rs:218-241). Make v3 default and remove legacy decoders when safe.

6) Renderer boundaries
- Doc: Renderer never mutates game state; uses replication only.
- Code: Default builds meet this; renderer provides `wizard_positions()` for platform demo AI (crates/render_wgpu/src/gfx/mod.rs:562-580). Legacy client AI/combat code remains behind features.
- Deviation: Remove legacy features once HUD/UI are fully replication‑driven (now mostly true with `HudStatusMsg`).

Summary
- Alignment with the refactor doc is strong on the server’s ECS, systems, and replication. Remaining work is chiefly removing legacy code and finishing the last mile (input intents for wizard movement, incremental spatial grid, remove compatibility decoders).
