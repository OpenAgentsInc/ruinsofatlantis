# 95C — ECS Components: Destructible, VoxelProxy, ChunkDirty, ChunkMesh, CarveRequest

Labels: ecs, server-authoritative, voxel
Depends on: Epic #95, 95B (Scaffolds)

Intent
- Define core components to carry destructible state and chunked geometry through ECS systems and replication.

Outcomes
- Components compile with rustdoc; simple unit test constructs an entity with a ChunkMesh map entry.
 
Status: COMPLETE (components landed; unit tests pass; exported from ecs_core)

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
 - [ ] Add serde + hashing where replication will need it:
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
   pub struct DestructibleId(pub u64);
   pub type ChunkKey = (u32,u32,u32);
   ```
 - [ ] Provide a helper for consistent keys:
   ```rust
   pub fn chunk_key(did: DestructibleId, c: glam::UVec3) -> (DestructibleId, u32,u32,u32) { (did, c.x, c.y, c.z) }
   ```
 - [ ] MeshCpu invariants and validation:
   ```rust
   impl MeshCpu {
       pub fn validate(&self) -> anyhow::Result<()> {
           anyhow::ensure!(self.positions.len() == self.normals.len(), "pos/normal len mismatch");
           anyhow::ensure!(self.indices.len() % 3 == 0, "indices not multiple of 3");
           Ok(())
       }
   }
   ```

Acceptance
- Components available to `server_core`/`client_core`/`render_wgpu`; unit test passes.
 - All new types derive `serde::{Serialize,Deserialize}` (optionally behind a `replication` feature if you prefer a lean default).

---

## Addendum — Implementation Summary (95C landed)

Implemented in `ecs_core`:
- Added `src/components.rs` with:
  - `EntityId`, `DestructibleId` (hashable IDs, serde behind feature `replication`).
  - `Destructible { id, material: core_materials::MaterialId }`.
  - `VoxelProxy { meta: voxel_proxy::VoxelProxyMeta }`.
  - `ChunkDirty(Vec<UVec3>)` (serde behind `replication`).
  - `MeshCpu { positions, normals, indices }` + `validate()` invariants.
  - `ChunkMesh { map: HashMap<(u32,u32,u32), MeshCpu> }` (serde behind `replication`).
  - `CarveRequest { did, center_m, radius_m, seed, impact_id }` (serde behind `replication`).
  - Helper `chunk_key(DestructibleId, UVec3)`.
- Exported via `pub mod components; pub use components::*;` in `ecs_core::lib`.
- Added deps: `serde` (derive), `anyhow`, `voxel_proxy`, `core_materials`.
- Unit test constructs a minimal `MeshCpu` and inserts into a `ChunkMesh` map, then validates.
- CI: clippy/tests green with new module; serde derives gated under `replication` feature.

