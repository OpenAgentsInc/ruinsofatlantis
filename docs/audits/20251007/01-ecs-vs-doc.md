# ECS Architecture vs Refactor Doc — 2025-10-07

Source of truth: docs/issues/ecs_refactor_part_2.md

What the doc specifies vs what’s implemented now

1) Replace ActorStore with ECS world
- Doc: Use `WorldEcs { world: ecs_core::World, net_map }`; keep public shape stable.
- Code: `server_core::ecs::WorldEcs` (custom, minimal ECS specialized for actors) replaces vec store inside `ServerState` (crates/server_core/src/lib.rs:104,127). It holds `Components { id, kind, team, tr, hp, … }` and offers `spawn/get/iter` (crates/server_core/src/ecs/mod.rs:33-41, 59-68).
- Deviation: `ActorStore` type still exists (not used by `ServerState`) (crates/server_core/src/actor.rs:58). WorldEcs does not wrap `ecs_core::World` as the doc suggested; it’s an internal specialized store.

2) System schedule with events
- Doc: Ordered systems (Input, AISelectTarget, AIMove, Melee, ProjectileIntegrate, ProjectileCollision, AoE, Faction, ApplyDamage, Cleanup, Snapshot).
- Code: `server_core::ecs::schedule::Schedule::run` executes: boss seek, undead move, melee apply (with cooldown), projectile integrate, projectile collision, AoE apply, faction flip on PC→Wizards, apply damage, cleanup (crates/server_core/src/ecs/schedule.rs:39-53). Damage and explosions flow through `DamageEvent`/`ExplodeEvent` (crates/server_core/src/ecs/schedule.rs:12-24, 26-35).
- Deviation: Input system is hosted in the platform loop (command drain + server projectile spawns) rather than an in‑ECS input system. Acceptable short‑term; still aligns with “server applies commands”.

3) Components for data (not constants)
- Doc: MoveSpeed, Melee, AggroRadius, AttackRadius (and optional Homing).
- Code: Present as optional fields on Components; used by AI/move/melee systems (crates/server_core/src/ecs/world.rs:35-43; schedule.rs:73-81, 96-107, 116-139).
- Deviation: No Homing yet; fine for MVP.

4) Spatial index
- Doc: 2D XZ uniform grid; use for proximity/collision/AoE.
- Code: `SpatialGrid` present and rebuilt once per tick; used by systems but projectile broad‑phase still linearly scans actors for now, then uses proximity explode (crates/server_core/src/ecs/schedule.rs:323-364, 171-213, 215-238).
- Deviation: Incremental updates and broad‑phase usage for segments are not fully exploited yet.

5) Replication: actor snapshots and deltas with interest
- Doc: ActorSnapshot v2, per‑client interest, and v3 deltas with baseline.
- Code: Present. Platform drains commands, steps server, builds interest‑limited view and v3 deltas against a baseline; sends framed bytes over loopback (crates/platform_winit/src/lib.rs:240-620). Net layer provides encode/decode and caps (crates/net_core/src/snapshot.rs).
- Deviation: Client still includes compatibility decoders for `NpcListMsg` and `BossStatusMsg` (crates/client_core/src/replication.rs:162-180).

6) Renderer boundaries
- Doc: Renderer never mutates game state; uses replication only.
- Code: Default builds meet this. Legacy paths for client‑side AI/combat are feature‑gated and off by default (crates/render_wgpu/Cargo.toml: features). `wizard_positions()` is provided for server demo AI only (crates/render_wgpu/src/gfx/mod.rs:562-580).
- Deviation: Legacy code remains in tree (behind features) and some renderer modules import server types under those features (e.g., npcs.rs:14, vox_onepath.rs). Safe by default, but should be deleted once migration stabilizes.

Summary
- Alignment with the refactor doc is strong on the server’s ECS, systems, and replication. Remaining work is chiefly removing legacy code and finishing the last mile (input intents for wizard movement, incremental spatial grid, remove compatibility decoders).

