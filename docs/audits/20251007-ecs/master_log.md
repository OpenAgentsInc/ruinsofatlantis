ECS Refactor — 2025-10-07

Scope
- Finish v4 replication cutover (remove v3 decoding and legacy messages).
- Normalize HitFx via Ctx and confirm schedule order.
- Neutralize archetype nouns in system identifiers.
- Cache specs in ServerState (no per-call loads).
- Expand CI guards; update docs.

Changes
- net_core
  - ActorSnapshotDelta is v4-only at runtime. Decoder rejects v!=4. Encoder always writes v4 spawn layout (kind, faction, archetype_id, name_id, unique).
  - Removed legacy BossStatusMsg/NpcListMsg tests; kept HudStatus.
  - Fixed delta roundtrip test to use `faction`/archetype fields.
- client_core
  - Renamed test to v4_updates_drive_views.rs and function to v4_*; updated replication comment to v4.
  - Added logic-only tests: archetype_maps_model.rs (archetype_id → model bucket) and anim_state_from_delta.rs (Idle/Jog/Death selection).
- server_core
  - Schedule: verified canonical order; wrapped spans without changing behavior.
  - Renamed systems: ai_wizard_cast_and_face → ai_caster_cast_and_face; ai_move_hostiles_toward_wizards → ai_move_hostiles.
  - HitFx already flows via Ctx; ServerState drains ctx.fx_hits after tick.
  - Cached specs: added `specs_arche` and `specs_proj` to ServerState; load in new(); replaced load_default() call sites.
  - Neutralized archetype nouns in test names; added cast_acceptance.rs covering GCD/per-spell cooldown and mana gating.
  - Caster AI prefers melee in reach; DK now reliably closes and lands melee instead of kiting at close range.
  - Added tests: pc_mana_regen.rs (1/s regen), cast_reject_toast.rs (HUD toast on insufficient mana), death_knight_engages.rs (moves closer and attacks).
- xtask
  - Expanded forbidden patterns: block Team (type name) in runtime crates; block legacy msgs; block v:\s*3 in net_core/client_core; keep ActorKind branching guard in server systems.
- docs
  - Updated docs/ECS.md to reflect v4 schema, system names, and client apply semantics.
  - Expanded ECS.md with explicit note: renderer selects model/rig solely by archetype_id; added Faction rules box.
  - Added HUD toast message to replication docs; documented that wizard bars use replicated HP.

Verification
- rg checks: no srv.fx_hits.push in systems; no BossStatusMsg/NpcListMsg in runtime; no v3 decoder acceptance.
- cargo clippy/test pass locally; xtask ci guards enabled for v4 only.
 - Renderer consumes replicated HitFx and spawns a small spark burst at each impact.
 - Wizard HP bars are built from replication (repl_buf.wizards) and drop on AoE.

Next
- Broaden neutral naming in tests (wizard → caster/melee_hostile) across server_core tests (non-functional rename).
- Optional: docs for archetype_id-driven model mapping in renderer.
- Implemented neutral helper rename (wizard_targets → targets_by_faction) and strengthened CI guards to prevent regressions.
