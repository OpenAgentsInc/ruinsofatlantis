# Simulation/Server Audit (`crates/sim_core`, `crates/server_core`)

Context
- Deterministic sim core; server_core hosts simple AI and destructible helpers.
- Client mutates world; no server authority, replication, or interest management yet.

Risks
- Hard‑coded NPC/ability constants; stringly flows for some IDs.
- No tick phasing or ECS systems for major features (projectiles, destructible carve).

Recommendations
1) ECS components
- `Transform`, `Velocity`, `Projectile`, `Health`, `Team`, `CollisionShape`, `Destructible`, `VoxelProxy`, `ChunkDirty`, `ChunkMesh`, `Debris`.

2) Systems (server tick)
- Fixed‑dt `ProjectileIntegrate`, `Collision`, `DamageApply`.
- `DestructibleRaycast` → `CarveRequest`; `VoxelCarve` → `ChunkDirty`.
- `GreedyMesh` and `ColliderRebuild` (budgeted).
- `DebrisSpawn` and optional `DebrisIntegrate`.

3) Server authority and replication
- Authoritative tick applies systems and emits snapshots/deltas.
- Spatial interest (grid) filters entities/chunks per client.
- Client prediction/reconciliation for movement/projectiles.

4) Data‑driven specs
- Move projectile/NPC constants to `data_runtime`; central `SpecDb`.

5) Tests
- Tick harness: run N ticks; assert counts/health/dirty chunk tails.
