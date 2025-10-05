# ECS & Data Model — 2025-10-04

Inventory
- Components for destructibles, voxel proxy, per-chunk meshes, carve requests, and HUD/boss attributes (crates/ecs_core/src/components.rs).
- `ChunkMesh { map: HashMap<(u32,u32,u32), MeshCpu> }` stores per-chunk CPU meshes; `MeshCpu` validates invariants (`positions.len == normals.len`, indices multiple-of-3).

Observations
- Authoritative mutation should remain server-side; clients consume `ChunkMesh` for upload.
- Be cautious when iterating over `HashMap` maps in authoritative loops — sort keys for stable order.

Recommendations
- Keep presentation-only components separated from authoritative ones; ensure server-to-client replication types are versioned and sized conservatively.
- Add size awareness in hot-path components (e.g., avoid large vectors on high-frequency components).

