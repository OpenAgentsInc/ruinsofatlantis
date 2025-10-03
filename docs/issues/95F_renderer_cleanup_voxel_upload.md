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
- [ ] Extract the VB/IB creation code path currently in `process_one_ruin_vox(..)` into:
  ```rust
  pub fn upload_chunk_mesh(
      device: &wgpu::Device,
      did: crate::gfx::DestructibleId,
      chunk: (u32,u32,u32),
      mesh: &MeshCpu,
      out_meshes: &mut HashMap<(crate::gfx::DestructibleId,u32,u32,u32), VoxelChunkMesh>,
      out_hashes: &mut HashMap<(crate::gfx::DestructibleId,u32,u32,u32), u64>,
  ) -> anyhow::Result<()>;
  ```
- [ ] Add `remove_chunk_mesh(..)` to evict empty chunks (drop VRAM/indices and hashes).
- [ ] Replace all direct `voxel_meshes.insert/remove` and `voxel_hashes.insert/remove` with helper calls; use a `chunk_key(did, UVec3)` helper to standardize keys.
- [ ] Add a CPU‑only unit test for `MeshCpu::validate()` and ensure upload helper bails on invalid data.

Compile Hygiene
- Keep legacy carve paths behind `legacy_client_carve`; annotate gated functions with `#[cfg_attr(not(feature = "legacy_client_carve"), allow(dead_code))]` to avoid `-D warnings` when disabled.
 - Gate demo helpers (`process_voxel_queues`, `build_voxel_grid_for_ruins`, `reset_voxel_and_replay`) behind `vox_onepath_demo` (see 95A).

Acceptance
- Default build: renderer only uploads/removes meshes via `voxel_upload`, with no world mutations.
- Feature build: legacy carve works as before; upload helper remains compatible.
- Smoke test: a single `MeshCpu` upload path renders a 1×1×1 cube (use a tiny CPU mesh in a test/dev hook).

---

## Addendum — Implementation Summary (95F partial)

- Added `crates/render_wgpu/src/gfx/renderer/voxel_upload.rs` with:
  - `upload_chunk_mesh(..)` that validates and uploads CPU mesh to VB/IB and updates renderer caches.
  - `remove_chunk_mesh(..)` to evict a chunk entry.
  - CPU-only test that `MeshCpu::validate()` catches mismatched lengths.
- Exported `voxel_upload` from `renderer/mod.rs`.
- Next: replace direct VB/IB creation sites in `update.rs` with the helper.
Status: PARTIAL (voxel_upload helper landed; call-sites to be migrated incrementally)
