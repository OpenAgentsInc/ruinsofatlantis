Gaps and ECS-Only Fix Plan

F-NPC-001 — NPC wizards do not cast (no ECS AI)
Severity: P0  Area: Server/ECS  Owner: server_core
Context: Wizards (Team::Wizards) are mirrored into ECS but never enqueue casts; `cast_system` only drains player-driven `pending_casts`.
Why it matters:
- Combat feels one-sided; undermines “all ECS” design goal.
Fix:
- Add `ai_wizard_cast_and_face(srv, ctx)` system before `cast_system`:
  - For each alive wizard (Team::Wizards) with `Spellbook`/`Cooldowns`, acquire nearest hostile (e.g., Undead/Boss) within range; set `tr.yaw` toward target.
  - Enqueue `CastCmd { caster: Some(id), pos: wand_muzzle_from_tr(tr), dir: to_target, spell: choose_spell() }` with gating (respect cooldowns/mana/stun).
Acceptance:
- In a demo scene with undead, NPC wizards periodically cast; projectiles replicate; hp reduces on hit.

F-NPC-002 — Boss (Nivita) never attacks (melee limited to Zombies)
Severity: P0  Area: Server/ECS  Owner: server_core
Context: `melee_apply_when_contact()` filters by `ActorKind::Zombie`; Nivita has `Melee` but is skipped.
Fix:
- Generalize melee to any actor with `Melee` component (and `MoveSpeed`/`AttackRadius` for reach). Filter by team/hostility rather than kind.
Acceptance:
- Nivita deals melee damage when in reach; cooldown applies; deaths/despawn timers behave.

F-NPC-003 — Death Knight not registered in ECS
Severity: P1  Area: Server/ECS  Owner: server_core
Context: DK exists only as renderer visuals; server never spawns/replicates any DK actor.
Fix (minimal):
- Add `spawn_death_knight(pos)` that creates an `ActorKind::Boss` (or introduce a boss subkind marker via data tag) with `MoveSpeed`/`Melee`.
- Platform demo: spawn DK once (similar to Nivita) and include in ECS tick.
- Replication: ensure it appears in actor list. Renderer can switch model based on a replicated tag/name.
Acceptance:
- DK appears as a replicated actor; moves/attacks like a boss.

F-NPC-004 — Movement AI split by kind
Severity: P1  Area: Server/ECS  Owner: server_core
Context: Zombies use `ai_move_undead_toward_wizards`; Nivita uses a separate `boss_seek_and_integrate`.
Fix:
- Replace with `ai_move_hostiles_toward_wizards(srv, ctx)`:
  - For any alive actor with `MoveSpeed` + `AggroRadius` (and hostile to Wizards), step toward nearest wizard until contact.
  - Optionally keep a boss-specific override later if behavior diverges.
Acceptance:
- Both zombies and Nivita advance using the same system; code is component-driven.

F-NPC-005 — `sync_wizards()` still mirrors player positions
Severity: P1  Area: Server/Platform  Owner: server_core, platform_winit, net_core
Fix:
- Implement authoritative intents per ecs_refactor_part_3.md:
  - net_core: add `ClientCmd::Move/Aim`; server ECS: add `IntentMove/IntentAim` + `input_apply_intents` (first in schedule).
  - platform: stop calling `sync_wizards()`; send inputs as intents.
Acceptance:
- PC moves and aims with server authority; respawn policy handled by ECS.

F-NPC-006 — Boss/DK subkind missing for visuals
Severity: P2  Area: Net/Client  Owner: net_core, render_wgpu
Fix:
- Add an optional `boss_subkind: u8` or name tag to `ActorRep` (or a sidecar BossRep) so renderer can choose DK vs Nivita model.
Acceptance:
- Renderer draws DK vs Nivita correctly using replicated data only.

Notes
- Projectile collision/arming delays are already ECS-based and working per prior refactor.
- Spatial grid currently rebuilds per tick; consider incremental later.

