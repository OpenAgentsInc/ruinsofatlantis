# 95F — Renderer Cleanup: Remove Client Mutation + Add voxel_upload Module

Labels: renderer, refactor
Depends on: Epic #95, 95A (Preflight gates)

Intent
- Ensure the renderer no longer mutates world state; introduce a dedicated upload helper for chunk meshes to prepare for replication.

Outcomes
- Client build (default) does not compile client-side carve/collider/mesh/debris paths; a new `voxel_upload` module handles VB/IB creation from CPU mesh input.

Files
- `crates/render_wgpu/src/gfx/renderer/update.rs` — client mutation calls gated (95A); keep ingestion stub for uploads.
- `crates/render_wgpu/src/gfx/renderer/voxel_upload.rs` (new) — helper to upload/remove a single chunk mesh.
- `crates/render_wgpu/src/gfx/renderer/init.rs` — call helper where necessary (e.g., on initial demo/proxy uploads if feature‑flagged).
- `crates/render_wgpu/src/gfx/renderer/render.rs` — unchanged draw loop.
 - Keys and caches already present in `Renderer`:
   - Maps: `voxel_meshes: HashMap<(DestructibleId,u32,u32,u32), VoxelChunkMesh>`, `voxel_hashes: HashMap<(DestructibleId,u32,u32,u32), u64>`; standardize access via a `chunk_key(did, UVec3)` helper.

Tasks
- [x] Extract the VB/IB creation code path into `voxel_upload::upload_chunk_mesh(..)`.
- [x] Add `remove_chunk_mesh(..)` to evict empty chunks (drop VRAM/indices and hashes).
- [x] Replace direct `voxel_meshes.insert/remove` and `voxel_hashes.insert/remove` with helper calls where chunk meshes are created.
- [x] Add a CPU‑only unit test for `MeshCpu::validate()` and ensure upload helper bails on invalid data.

Compile Hygiene
- Keep legacy carve paths behind `legacy_client_carve`; gate demo helpers under `vox_onepath_demo`.

Acceptance
- Default build: renderer only uploads/removes meshes via `voxel_upload`, with no world mutations.
- Feature build: legacy carve works as before; upload helper remains compatible.
- Smoke: helper compiles and unit test validates CPU mesh invariants.

---

## Addendum — Implementation Summary (95F COMPLETE)

- Added `crates/render_wgpu/src/gfx/renderer/voxel_upload.rs` with:
  - `upload_chunk_mesh(..)` that validates and uploads CPU mesh to VB/IB and updates renderer caches.
  - `remove_chunk_mesh(..)` to evict a chunk entry.
  - CPU-only test that `MeshCpu::validate()` catches mismatched lengths.
- Exported `voxel_upload` from `renderer/mod.rs`.
- Replaced direct VB/IB creation sites in `renderer/update.rs` with the helper for both legacy ruin queues and the one‑path demo path.
- Preserved occupancy‑hash skip optimization by overriding the uploaded hash with the grid’s `chunk_occ_hash` after upload.
Status: COMPLETE
