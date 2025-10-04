# 95L — Scene Build (Server): Data-Driven Destructible Registry

Status: IN PROGRESS

Labels: scene, data, server-authoritative
Depends on: Epic #95, 95C (Components), 95D (Data), 95I (Replication)

Intent
- Move destructible seeding from client renderer to server scene build and make tagging data-driven.

Outcomes
- Server assembles destructible entities from scene data; client consumes replication (no client GLTF loads for destructibles).

Files
- `crates/server_core/src/scene_build.rs` (new)
- `crates/data_runtime/src/schemas/scene_destructibles.json` (or TOML)
- `crates/render_wgpu/src/gfx/scene.rs` — remove client-side destructible seeding; consume replication for visuals
 - Current client seeding (to replace): `crates/render_wgpu/src/gfx/scene.rs` loads `assets/models/ruins.gltf`, computes local AABB and per-instance world AABBs, and seeds `DestructInstance` on the client. This logic should move server-side and replicate entities to the client.

Tasks
- [x] Load scene/zone data and destructible tags; build per-instance records (data_runtime::scene::destructibles).
- [x] Compute per-instance world AABBs (transform 8 corners of local AABB); `server_core::scene_build::world_aabb_from_local` and helper to build instances.
- [x] Define net_core snapshot record for destructible instances (world AABBs) with encode/decode + tests.
- [ ] Replicate these to client; hide source instance on first hit via replicated event.
- [ ] Remove client GLTF reload for destructibles; only build GPU buffers from replicated CPU instance data.

Acceptance
- Any tagged mesh (not just ruins) is destructible; renderer no longer loads GLTF or computes AABBs for destructibles.
 - Renderer compiles with destructible seeding removed; visuals still reflect replicated registry.

Addendum (this pass)
- Added `data_runtime::scene::destructibles` with TOML loader and tests for a minimal scene format.
- Implemented `server_core::scene_build::build_destructible_instances` to compute world AABBs from scene declarations (unit-tested).
- Extended `net_core::snapshot` with `DestructibleInstance` (did + world_min/max) and a round-trip test.
- Next: wire a server stub to emit `DestructibleInstance` records via `net_core::channel::Tx`, and a client-side apply to build visuals without GLTF loads.
