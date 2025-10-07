2025-10-07 — NPC ECS refactor implementation log

Changes implemented (per audit 20251007)
- Movement (ECS, component-driven)
  - Replaced zombie-only movement with `ai_move_hostiles_toward_wizards` over any actor with `MoveSpeed` + `AggroRadius` hostile to Wizards.
  - Removed schedule call to `boss_seek_and_integrate`; boss now moves via the unified system.

- Melee (ECS, component-driven)
  - Generalized `melee_apply_when_contact` to include any hostile actor with `Melee` component (not just Zombies). Boss/Nivita now attacks in reach.

- NPC Wizard casting AI
  - Added `ai_wizard_cast_and_face`: Wizards (Team::Wizards) face nearest hostile and enqueue `CastCmd` (Firebolt). Ensures `Spellbook`, `Cooldowns`, and `ResourcePool` exist; pre-checks GCD/CD/mana to reduce queue churn. `cast_system` retains authoritative gating.
  - `sync_wizards()` now attaches casting components to newly spawned NPC wizards.

- Intents scaffolding (authoritative inputs)
  - net_core: added `ClientCmd::Move { dx,dz,run }` and `ClientCmd::Aim { yaw }` with encode/decode.
  - server_core ECS: added `IntentMove`/`IntentAim` components and `input_apply_intents` system (first in schedule) to integrate movement and yaw; added `ServerState::apply_move_intent`/`apply_aim_intent`.
- platform_winit: handles new Move/Aim commands and forwards to server intents.
- Note: renderer still mirrors PC positions via `sync_wizards()`; intents path is ready for full cutover when client starts emitting Move/Aim.

- Mirroring cutover (partial)
  - server_core: step_authoritative no longer mirrors wizard positions; added `spawn_pc_at(pos)` and kept `sync_wizards()` only for tests/compat.
  - platform_winit: on startup, spawns PC via `spawn_pc_at` instead of `sync_wizards`; continues to pass wizard positions only for interest culling.
  - renderer: now emits Move/Aim commands each frame (camera-relative movement, current yaw) alongside existing cast commands.

- Death Knight ECS registration
  - server_core: added `spawn_death_knight(pos)` (ActorKind::Boss, Team::Undead) with movement/melee components.
  - platform_winit demo: spawn a Death Knight on startup alongside Nivita.

- Boss/NPC simplification (no bespoke logic)
  - Movement/melee/casting are component-driven; no boss-only systems remain.
  - Added optional `name: Option<String>` component to ECS `Components`; set for Nivita and Death Knight.
  - BossStatus/HUD remains for now (backwards-compatible). Plan to migrate to generic name/hp from replicated actors in a follow-up.

Schedule order updated
1) input_apply_intents
2) cooldown_and_mana_tick
3) ai_wizard_cast_and_face
4) cast_system → ingest_projectile_spawns → apply_cmds
5) spatial.rebuild
6) effects_tick
7) ai_move_hostiles_toward_wizards
8) melee_apply_when_contact
9) homing_acquire_targets → homing_update
10) projectile_integrate_ecs → projectile_collision_ecs
11) aoe_apply_explosions → apply_damage_to_ecs → cleanup

Files touched
- server_core/src/ecs/world.rs: added intents to Components; new IntentMove/IntentAim types.
- server_core/src/ecs/schedule.rs: new systems (input_apply_intents, ai_wizard_cast_and_face); movement/melee generalized; schedule order updated.
- server_core/src/lib.rs: NPC wizard components on spawn; added apply_move_intent/apply_aim_intent; added spawn_death_knight.
- net_core/src/command.rs: Move/Aim command variants.
- platform_winit/src/lib.rs: demo spawn of DK; decode Move/Aim and forward to server.

Notes
- Boss helper `boss_seek_and_integrate` remains for its unit test but is no longer called by the schedule.
- Full intents cutover (removing `sync_wizards()`) will be done once the client emits Move/Aim; current path maintains demo functionality.

Next (optional follow-up)
- Client: emit Move/Aim per ecs_refactor_part_3.md to complete the mirroring removal.
- Net schema: add boss subkind/name tag for DK vs Nivita visuals.
- Removed bespoke boss code
  - Deleted `crates/server_core/src/systems/boss.rs` and removed it from `systems/mod.rs`.
  - All behavior (movement/melee/casting) now flows through generic ECS systems.

- Renderer clean-up (no client melee demo)
  - Removed calls to `apply_zombie_melee_demo()` and `update_sorceress_motion()` from the frame loop; server authority only.
  - Left legacy bodies commented behind `cfg(any())` to maintain structure without dead-code warnings.

- Tests
  - Added `server_core/tests/fireball_aoe_hits_wizards.rs` ensuring Fireball AoE reduces clustered wizard HP.

- Demo spawns (NPC wizards)
  - platform_winit: spawn a small circle of NPC wizard casters near center so Undead don’t only target the PC; Zombies now also engage NPC wizards.

- Warnings cleanup
  - Fixed unused parameter (`wizard_positions` → `_wizard_positions`) in `Schedule::run`.
  - Silenced dead-code warnings by removing legacy calls and stubbing legacy-only methods under cfg.
