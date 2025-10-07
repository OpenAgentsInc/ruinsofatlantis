### F-NPC-001 — NPC wizards never cast
**Severity:** P0  **Confidence:** High  **Area:** Server/ECS
**Context:** Wizards (Team::Wizards) are mirrored but no ECS AI enqueues casts.
**Evidence:** crates/server_core/src/ecs/schedule.rs:322 (cast_system drains pending_casts only); crates/server_core/src/lib.rs:258 (sync_wizards)
**Why it matters:** Server authority requires all actions via ECS; combats feel dead without enemy casters.
**Recommendation:** Add `ai_wizard_cast_and_face` before `cast_system`; choose spell, face target, enqueue `CastCmd` with gating.
**Status:** Fixed (ai_wizard_cast_and_face added; NPC wizards cast Firebolt).
**Effort:** M  **Owner:** server_core

### F-NPC-002 — Nivita never attacks
**Severity:** P0  **Confidence:** High  **Area:** Server/ECS
**Context:** Nivita has `Melee` but `melee_apply_when_contact` filters by `ActorKind::Zombie`.
**Evidence:** crates/server_core/src/ecs/schedule.rs:461 (zombie-only filter); crates/server_core/src/lib.rs:488 (Nivita components include Melee)
**Why it matters:** Boss must deal damage for encounter to function.
**Recommendation:** Generalize melee to any actor with `Melee` component; filter by hostility, not kind.
**Status:** Fixed (melee_apply_when_contact generalized; Nivita now attacks).
**Effort:** S  **Owner:** server_core

### F-NPC-003 — Death Knight not an ECS actor
**Severity:** P1  **Confidence:** High  **Area:** Server/ECS
**Context:** Renderer loads DK visuals; server never spawns a DK actor.
**Evidence:** crates/render_wgpu/src/gfx/deathknight.rs (assets); no server_core spawn for DK in repo greps.
**Why it matters:** Non-replicated visuals break authority and determinism.
**Recommendation:** Add server spawn (`spawn_death_knight`) and treat as Boss (or tag subkind); include in replication.
**Status:** Fixed (spawn_death_knight added; spawned in demo).
**Effort:** M  **Owner:** server_core

### F-NPC-004 — Movement AI split by kind
**Severity:** P1  **Confidence:** High  **Area:** Server/ECS
**Context:** Zombies use a system; Nivita uses `boss_seek` helper.
**Evidence:** crates/server_core/src/ecs/schedule.rs:406; crates/server_core/src/systems/boss.rs:9
**Why it matters:** Duplicate logic, inconsistent behaviors; not component-driven.
**Recommendation:** Replace with a unified `ai_move_hostiles_toward_wizards` over `MoveSpeed`+`AggroRadius`.
**Status:** Fixed (ai_move_hostiles_toward_wizards added; boss helper removed from schedule).
**Effort:** M  **Owner:** server_core

### F-NPC-005 — Player mirroring (no intents)
**Severity:** P1  **Confidence:** High  **Area:** Server/Platform
**Context:** Platform still calls `sync_wizards()`; intents not present.
**Evidence:** crates/platform_winit/src/lib.rs:117 (sync_wizards); ecs_refactor_part_3.md requires intents.
**Why it matters:** Server must own transforms; mirroring undermines authority.
**Recommendation:** Implement `ClientCmd::Move/Aim` + `IntentMove/IntentAim` and `input_apply_intents`; remove mirroring.
**Status:** Implemented (Move/Aim commands + intents + renderer emit). Mirroring removed from step; platform uses spawn_pc_at at startup. Full client-side Move/Aim emission is active; tests keep sync helper.
**Effort:** M  **Owner:** net_core, server_core, platform_winit

### F-NPC-006 — Boss/DK visual subkind missing
**Severity:** P2  **Confidence:** Medium  **Area:** Net/Client
**Context:** ActorRep has `kind` but no boss subtype to distinguish Nivita vs DK in renderer.
**Evidence:** crates/net_core/src/snapshot.rs:315 (ActorSnapshotDelta/ActorRep schema)
**Why it matters:** Renderer may draw wrong model or need server fallback.
**Recommendation:** Add subkind or name tag for bosses; renderer uses it to select assets.
**Status:** Pending (next).
**Effort:** S  **Owner:** net_core, render_wgpu
