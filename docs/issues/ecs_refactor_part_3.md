# ECS Refactor — Part 3 Addendum (2025-10-07)

This addendum tracks the hard cut to ECS/server‑authority and the fixes applied today to stabilize projectiles, collisions, and visual feedback.

## Summary

- Server: projectiles now collide with any actor (skip owner only); Fireball has a short arming delay to avoid instant detonation; spawns offset forward to prevent self‑collision.
- Client: Fireball always shows explosion VFX and spawns damage floaters over NPCs and Wizards within AoE in default builds.
- Logs: verbose snapshot/cast/projectile logs gated behind `RA_LOG_*` envs by default.
- PC resiliency: server auto‑respawns PC actor in `sync_wizards()` if dead/missing so casts stay valid.

## Files changed

- `crates/server_core/src/ecs/schedule.rs`
  - Removed faction gating in `projectile_collision_ecs` (direct + AoE proximity); added arming delay (FB: 0.18s, others: 0.08s); small spawn offset applied elsewhere.
- `crates/server_core/src/lib.rs`
  - `sync_wizards()` respawns PC if missing/dead; snapshot v2 logging behind `RA_LOG_SNAPSHOTS`; enqueue_cast logging behind `RA_LOG_CASTS`.
- `crates/render_wgpu/src/gfx/renderer/render.rs`
  - Tracks last replicated projectiles; spawns explosion VFX on Fireball disappear.
- `crates/render_wgpu/src/gfx/renderer/update.rs`
  - `explode_fireball_at()` now spawns damage floaters (default build) for NPCs and Wizards.
  - Renderer projectile count logging behind `RA_LOG_PROJECTILES`.

## Acceptance (observed)

- Firebolt/MM/Fireball travel and collide; projectiles replicated consecutively (v2 by default) before removal.
- Fireball disappearance triggers VFX + floaters even when removal was server‑side.
- Wizards take damage server‑side; client shows floaters above Wizards.
- Logs are quiet by default; enable with `RA_LOG_CASTS=1`, `RA_LOG_SNAPSHOTS=1`, `RA_LOG_PROJECTILES=1` when needed.

## Next (tracked separately)

- Remove legacy client‑side AI/combat/features; replication v3 only; intents instead of `sync_wizards()`; incremental spatial grid.

