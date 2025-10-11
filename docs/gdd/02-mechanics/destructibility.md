# Destructibility (Structures & Environment)

Design intent
- Make the world malleable and reactive; destructibility should add depth, customizability, and fun to roleplay and combat.
- Default stance: the world is destructible by default; specific areas are protected by explicit wards (safe zones).
- Balance ambient, small‑scale damage (sledgehammer dents a gate) with dramatic set‑pieces (dragonfire topples structures, boss punches through walls).

Scope and priorities (v0 → v1)
- Start with structures, not terrain: focus on buildings, walls, gates, props. Defer generic terrain edits (grief‑resistant, easier to author and reason about).
- Context‑aware: enable full damage in sieges/combat zones and the open world; protect towns/markets via ward volumes (laws/wards, not a global “no damage” flag).
- Ambient destructibility: outside wards, small hits should leave marks/dents; within wards, damage is blocked or redirected.
- Persistence: zone policies define if/when repairs occur (automatic, scheduled, or player‑driven via tools/spells).

Rules of engagement
- Wards and safe zones
  - Ward volumes mark protected areas (cities, quest hubs). Display a clear indicator (HUD/tooltip).
  - Wards are intentional game objects/systems; they can be configured, powered, or sabotaged (future).
- Structure vs. terrain
  - Phase 1: structure damage only (buildings/props/gates). Phase 2+: carefully evaluate limited terrain edits.
- Griefing mitigation
  - Repair loops (tools + Mending), crime/law consequences, resource gating, cooldowns/budgets, and persistence policies per zone.
- PvP and law
  - Destruction in protected areas triggers guards/outlaw status; in war/siege contexts, rules relax by design.

Authoring and data
- Mark destructible instances in zone authoring data (class/material/voxel proxy). Prefer pre‑baked voxel proxies for large structures.
- Materials: use `core_materials` densities and albedo to derive mass/debris.
- Budgets: per‑tick carve/mesh/collider budgets to keep simulation stable.

Runtime and replication
- Server‑authoritative: server applies damage (carves), rebuilds meshes/colliders within budgets, and replicates chunk/delta updates.
- Client: uploads changed chunks to GPU (via `client_core::upload` and renderer zone batches). No gameplay parameters client‑side.
- Metrics: track carve rate, budgets, mesh/collider rebuild time, and snapshot bytes.

Examples and fantasies
- Dragonfire over a city melts roof tiles and topples weak walls outside wards.
- Boss punch breaks through a tower segment during a siege set‑piece.
- A player’s sledgehammer visibly dents a city gate (outside wards), progressing toward a breach.

Phased implementation
- Phase 1 (target after core mechanics)
  - Extend existing destructible pipeline to building classes; bake voxel proxies for select structures.
  - Add ward volumes and default city protections; show HUD indicators.
  - Replicate structure damage via existing snapshot deltas; integrate repair jobs (tools + Mending) for recovery.
- Phase 2
  - Siege content: zone logic that authorizes destructibility broadly during declared wars.
  - Debris sampling/FX, improved materials, and performance tuning.
  - Optional limited terrain edits with strict safeguards.

Testing and feedback
- Unit tests for server carve/mesh/collider order and budgets.
- Golden tests for voxel chunk deltas and replication size.
- Structured playtests to calibrate “how much is fun” for ambient vs. siege‑level destruction.

Related systems
- Tech notes: see `docs/gdd/11-technical/destructibles/status.md` for current pipeline status and plans.
- Zones: see `docs/gdd/08-zones-cosmology/zones-system.md` for authoring, bake, and runtime loading.
