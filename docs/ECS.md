ECS Overview (Server Authority)

Scope
- Summarizes the current server-core ECS: actor identity, components, systems, and where they live.
- Documents how platform/renderer interact (commands, replication) with filepaths for quick navigation.

Entities (Actor model)
- `server_core::ecs::world::Components` (crates/server_core/src/ecs/world.rs)
  - Identity: `id: ActorId`, `kind: ActorKind` (Wizard | Zombie | Boss [label only]), `faction: Faction` (Pc | Wizards | Undead | Neutral)
  - Optional: `name: Option<String>` (presentation-only; set for “Nivita”, “Death Knight”)
  - Transform/Health: `tr: Transform { pos, yaw, radius }`, `hp: Health { hp, max }`
  - Movement/Targeting: `move_speed: MoveSpeed { mps }`, `aggro: AggroRadius { m }`, `attack: AttackRadius { m }`
  - Melee: `melee: Melee { damage, cooldown_s, ready_in_s }`
  - Projectiles: `projectile: Projectile { kind, ttl_s, age_s }`, `velocity: Velocity { v }`, `owner: Owner { id }`, `homing: Homing { target, turn_rate, max_range_m, reacquire }`
  - Casting: `spellbook: Spellbook { known }`, `pool: ResourcePool { mana, max, regen_per_s }`, `cooldowns: Cooldowns { gcd_s, gcd_ready, per_spell }`
  - Effects: `burning: Burning`, `slow: Slow`, `stunned: Stunned`
  - Lifecycle: `despawn_after: DespawnAfter { seconds }`
  - Intents: `intent_move: IntentMove { dx, dz, run }`, `intent_aim: IntentAim { yaw }` (consumed each tick)

World
- `server_core::ecs::world::WorldEcs` (crates/server_core/src/ecs/world.rs)
  - Stores `Vec<Components>`, spawns, iteration helpers, and a simple nearest_hostile utility.
  - `CmdBuf { spawns, despawns }` to apply spawns/removals in a batch per-tick.

Systems Schedule (server)
- `server_core::ecs::schedule::Schedule::run` (crates/server_core/src/ecs/schedule.rs)
  1) `input_apply_intents` — integrates `IntentMove/IntentAim` (server-auth movement/aim)
  2) `cooldown_and_mana_tick` — regen, gcd, per-spell CD decay
  3) `ai_caster_cast_and_face` — NPC caster ranged AI (choose Fireball/MM/Firebolt; gating respected)
  4) `cast_system` — drains `pending_casts` → spawn projectiles into `pending_projectiles`
  5) `ingest_projectile_spawns` → create ECS projectile entities; applyCmds
  6) `spatial.rebuild` — frame-wide grid rebuild (temporary; future: incremental)
  7) `effects_tick` — DoTs/slow/stun update and queue `DamageEvent`
  8) `ai_move_hostiles` — component-driven movement for any hostile with MoveSpeed+AggroRadius
  9) `melee_apply_when_contact` — generic melee on contact for any hostile with Melee
  10) `homing_acquire_targets` → `homing_update`
  11) `projectile_integrate_ecs` → `projectile_collision_ecs` (arming delay + AoE proximity)
  12) `aoe_apply_explosions` — translate ExplodeEvent to DamageEvent
  13) `apply_damage_to_ecs` — apply to HP, enqueue DeathEvent, set DespawnAfter
  14) `cleanup` — despawn per timers or dead without timers

Authoritative Commands & Intents
- net_core command wire (crates/net_core/src/command.rs)
  - ClientCmd::Move { dx, dz, run } — per-frame movement intent
  - ClientCmd::Aim { yaw } — per-frame aim intent
  - ClientCmd::{FireBolt, Fireball, MagicMissile} — cast requests (server validates)
- platform_winit (crates/platform_winit/src/lib.rs)
  - Drains command channel; forwards Move/Aim to `ServerState::apply_*_intent`; enqueues casts.
  - Steps server, builds v3 deltas, and sends replication frames to the renderer.
- renderer emits Move/Aim/Cast (crates/render_wgpu/src/gfx/renderer/render.rs)
  - Encodes Move/Aim every frame based on input + camera yaw; continues to send cast commands on keybinds.

Replication (server → client)
- ActorSnapshotDelta v4 (crates/net_core/src/snapshot.rs)
  - spawns: ActorRep { id, kind, faction (u8), archetype_id, name_id, unique, pos, yaw, radius, hp, max, alive }
  - updates: ActorDeltaRec (bitmask: pos/yaw/hp/alive)
  - removals: Vec<u32>
  - projectiles: full ProjectileRep list (id, kind, pos, vel)
  - hits: Vec<HitFx> — tiny impact events for VFX (no gameplay)
- Client buffer (crates/client_core/src/replication.rs)
  - Maintains: `actors`, `wizards` (subset), `npcs`, `projectiles`, `hits`, `hud`
  - Applies deltas and rebuilds derived views every v4 apply to ensure HP/pos/yaw sync.
  - Presentation: model/rig selection is keyed by `archetype_id` (data-driven), not `kind`.

ServerState entry points (demo & helpers)
- Spawning (crates/server_core/src/lib.rs)
  - `spawn_pc_at(pos)`, `spawn_undead(pos, radius, hp)`, `spawn_wizard_npc(pos)`, `spawn_death_knight(pos)`
  - `spawn_nivita_unique(pos)` — unique NPC via data_runtime config (behaves generically via systems)
- Casting/Projectiles
  - `enqueue_cast(pos, dir, spell)` → schedule drains and spawns projectiles
  - `pending_projectiles` queue → `ingest_projectile_spawns`
- Ticking
  - `step_authoritative(dt)` → runs schedule (server-authoritative; no mirroring)

Renderer (presentation-only)
- Replication-driven visuals; no gameplay logic in default build.
  - Wizards/zombies/DK/Sorceress palettes update: CPU sample and GPU upload
  - Projectiles/VFX: updated from replication; `HitFx` events spawn small impact bursts
  - Filepaths: crates/render_wgpu/src/gfx/{mod.rs,renderer/{render.rs,update.rs},deathknight.rs}

Demo Server (platform)
- Spawns rings of Undead, a circle of NPC wizards, Nivita (unique), and a DK for variety.
- Steps authority; sends v4 deltas (including `hits`) + HUD; accepts Move/Aim/Cast commands from the renderer.
- File: crates/platform_winit/src/lib.rs

Notes & Next
- Boss/Navita: no bespoke logic; behavior driven by components + shared systems.
- Suggested net_core extensions: add `archetype_id/name_id/unique` to ActorRep to decouple HUD/model selection from bespoke messages.
- Spatial grid is currently rebuilt per frame; consider incremental updates tied to Transform writes and expose `query_segment` for projectile broad-phase.
