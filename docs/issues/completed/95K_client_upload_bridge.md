# 95K — Client Upload Bridge: client_core → render_wgpu

Status: COMPLETE

Labels: client, renderer
Depends on: Epic #95, 95F (Renderer upload), 95I (Replication v0)

Intent
- Implement a thin bridge from client_core replication to render_wgpu's `voxel_upload` helper.

Outcomes
- Client receives `ChunkMeshDelta` and translates into GPU uploads/removes via a stable trait/interface.

Files
- `crates/client_core/src/upload.rs`
- `crates/render_wgpu/src/gfx/renderer/voxel_upload.rs`
 - Renderer caches to update: `Renderer::voxel_meshes`, `Renderer::voxel_hashes` (keys `(DestructibleId,u32,u32,u32)`).

Tasks
- [x] Define a `MeshUpload` trait in client_core and implement in renderer host to call `voxel_upload::upload_chunk_mesh/remove_chunk_mesh`.
- [x] Ensure dedupe and idempotency via standardized keys `(DestructibleId, chunk)`.
- [x] Unit test a fake uploader accepting one `ChunkMeshDelta` and asserting a tracked map change.
 - [x] Provide a helper `chunk_key(did, UVec3)` and use it for all insert/remove paths to avoid tuple drift.

Acceptance
- Mesh updates applied on client cause visible voxel surfaces to appear/update; removing delta removes GPU buffers.
 - Repeated deltas with unchanged hashes are skipped (optional optimization).

---

## Addendum — Implementation Summary

- `client_core::upload` defines `MeshUpload` and `ChunkMeshEntry`.
- `render_wgpu` implements `MeshUpload` for `Renderer` (adapter converts `ChunkMeshEntry` → `MeshCpu` and delegates to `voxel_upload`).
- `voxel_upload` stays the single point that edits `voxel_meshes`/`voxel_hashes` with the standardized key `(DestructibleId, u32, u32, u32)`.
- Test coverage uses the replication local loop to push a `ChunkMeshDelta` and assert an upload occurs via a mock uploader.
