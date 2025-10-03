# 95L — Scene Build (Server): Data-Driven Destructible Registry

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

Tasks
- [ ] Load scene/zone data and destructible tags; build `Destructible` + `VoxelProxyMeta` per instance.
- [ ] Compute per-instance world AABBs (transform 8 corners of local AABB); store on entities.
- [ ] Replicate these to client; hide source instance on first hit via replicated event.

Acceptance
- Any tagged mesh (not just ruins) is destructible; renderer no longer loads GLTF or computes AABBs for destructibles.
