# 95C — ECS Components: Destructible, VoxelProxy, ChunkDirty, ChunkMesh, CarveRequest

Labels: ecs, server-authoritative, voxel
Depends on: Epic #95, 95B (Scaffolds)

Intent
- Define core components to carry destructible state and chunked geometry through ECS systems and replication.

Outcomes
- Components compile with rustdoc; simple unit test constructs an entity with a ChunkMesh map entry.

Repo‑aware Inventory
- `crates/ecs_core/src/lib.rs` currently defines `Entity`, `Transform`, `RenderKind`, and a minimal `World` with arrays (no type‑erased components yet).
- For this phase, we add new component structs and expose them for downstream crates.

Files
- `crates/ecs_core/src/components.rs` (new) — add component structs
- `crates/ecs_core/src/lib.rs` — `pub mod components;` and re‑exports

Components
- `Destructible { id: u64, material: core_materials::MaterialId }`
- `VoxelProxy { meta: voxel_proxy::VoxelProxyMeta }`
- `ChunkDirty(pub Vec<glam::UVec3>)`
- `MeshCpu { positions: Vec<[f32;3]>, normals: Vec<[f32;3]>, indices: Vec<u32> }`
- `ChunkMesh { pub map: std::collections::HashMap<(u32,u32,u32), MeshCpu> }`
- `CarveRequest { did: u64, center_m: glam::DVec3, radius_m: f64, seed: u64, impact_id: u32 }`
- Newtypes: `EntityId(u64)`, `DestructibleId(u64)` (derive Clone/Copy/Hash/serde if needed)

Tasks
- [ ] Implement components with derives (`Debug`, `Clone`, `Default` where useful) and rustdoc for mutability/ownership (server vs client).
- [ ] Unit test creates a `ChunkMesh` with a single entry and verifies indexing.

Acceptance
- Components available to `server_core`/`client_core`/`render_wgpu`; unit test passes.
