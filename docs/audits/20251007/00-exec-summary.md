NPC ECS Audit — 2025-10-07

Scope
- Verify all NPCs (zombies, wizard NPCs, Nivita, Death Knight) are fully server-authoritative via the ECS.
- Identify any residual non‑ECS behavior and propose concrete ECS‑only fixes.

Summary
- Zombies: Fully ECS-driven. Spawned via `spawn_undead`, move toward wizards, and melee via ECS systems.
- NPC wizards: Exist as `ActorKind::Wizard` (Team::Wizards) mirrored from the platform, but do not cast. No ECS AI system enqueues their casts yet.
- Nivita (boss): Spawned as `ActorKind::Boss` with movement toward wizards. Has Melee components, but melee system only targets Zombies, so Nivita does not attack.
- Death Knight: Present as renderer visuals only. Not spawned/owned by the ECS; no behavior on server.
- Replication: Actor + projectile replication is ECS-based. HUD boss status comes from server.

Top Findings (P0–P1)
- F-NPC-001 (P0): No ECS wizard-casting AI. NPC wizards never fire.
- F-NPC-002 (P0): Melee system filters by `Zombie` kind; Boss (Nivita) never attacks.
- F-NPC-003 (P1): Death Knight has no ECS registration; renderer-only.
- F-NPC-004 (P1): Movement AI is split (zombies vs boss helper). Generalize to component-based movement.
- F-NPC-005 (P1): Player mirroring (`sync_wizards`) remains; intents not wired yet.

Outcomes Required to be “All ECS”
- Add ECS systems for NPC wizard spellcasting + facing.
- Include all hostile actors with `MoveSpeed`/`AggroRadius` in movement and melee, not just Zombies.
- Register Death Knight as a server actor (reuse Boss kind or add a subkind indicator for visuals).
- Replace `sync_wizards()` with authoritative intents.

Evidence
- See `evidence/rg-npc-core.txt`, `evidence/rg-replication.txt`, `evidence/rg-boss-dk.txt`.

