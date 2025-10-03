# 95I — Replication v0: Snapshots, Deltas, Interest (Local Loop)

Labels: networking, replication
Depends on: Epic #95, 95B (Scaffolds), 95E/95G (Server systems)

Intent
- Establish a minimal replication pipeline for dirty chunks and core components using in-proc messaging (no sockets yet).

Outcomes
- Server emits snapshot/delta messages for entities/components of interest; client applies them to ECS, triggering mesh uploads.

Files
- `crates/net_core/src/snapshot.rs` — message structs; encode/decode traits
- `crates/net_core/src/apply.rs` — apply messages to a client ECS world
- `crates/net_core/src/interest.rs` — simple spatial interest grid
- `crates/client_core/src/replication.rs` — glue to apply into client ECS and emit upload jobs
 - For local loop, source camera position from client renderer:
   - `crates/render_wgpu/src/gfx/renderer/update.rs` has `cam_follow.current_pos` and the PC transform; feed these to interest filtering.

Messages (initial)
- `EntityHeader { id: u64, archetype: u16 }`
- Components: `Transform`, `Health`, `Projectile`, `Destructible`, `VoxelProxyMeta`
- Chunk mesh: `ChunkMeshDelta { did: u64, chunk: (u32,u32,u32), positions: Vec<[f32;3]>, normals: Vec<[f32;3]>, indices: Vec<u32> }`

Tasks
- [ ] Define encode/decode skeletons in `net_core` (implementations can be naïve v0).
- [ ] Interest filter: entities/chunks within R meters of client camera; server ticks produce only those deltas.
- [ ] Client apply merges/creates components and forwards `ChunkMeshDelta` to `client_core::upload`.
 - [ ] Standardize keys with `DestructibleId` and a `ChunkKey` newtype; avoid ad-hoc tuple assembly.
 - [ ] Ingest `VoxelProxyMeta` so client can compute world AABB for basic culling if needed.

Acceptance
- In a local run, carving produces `ChunkMeshDelta` messages and client reflects visible changes without client-side mutation.
 - Interest radius reduces messages when camera is far; logs include counts of sent/received deltas under a `replication_debug` feature (optional).
