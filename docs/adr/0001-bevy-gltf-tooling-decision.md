# ADR-0001: Use Bevy `bevy_gltf` via Cargo in tools (no vendoring)

Date: 2025-10-19

## Status
Accepted

## Context
We need a reliable, feature-aware glTF/GLB importer for asset baking and inspection. Bevy’s `bevy_gltf` provides:
- ECS scene traversal + strong glTF mapping (nodes, skins, animations, materials)
- Mature texture/sampler handling and PBR extension coverage
- A documented plugin/loader system with per-load settings

Vendoring `bevy_gltf` into our repo would create maintenance overhead and drift vs upstream. We only need it for offline tools; our runtime keeps a custom renderer and content format.

## Decision
- Use `bevy_gltf` via Cargo inside a new tools crate: `tools/gltf-baker`.
- Do not vendor `bevy_gltf` sources into this repo.
- Keep runtime renderer unchanged; tools bake RoA-native packs (skinned mesh, clips, submeshes, baseColor, node index↔name map).
- Draco remains an offline preprocess step; fail fast when `KHR_draco_mesh_compression` is detected.

## Consequences
- Faster time-to-quality for glTF ingestion in tools.
- Clear boundary between runtime (custom renderer) and tooling (Bevy).
- No lock-in: we own the exported pack format and can evolve independently.

## Implementation Plan
- Create `tools/gltf-baker` (Bevy app w/ `GltfPlugin`, minimal DefaultPlugins).
- CLI: `cargo run -p gltf-baker -- <in.gltf|glb> <out.ron|bin>`.
- Export:
  - Skinned mesh (verts/joints/weights/uvs), inverse binds, joint order
  - Animation clips to our `AnimClip` (node index–based targets)
  - Submesh ranges + baseColor (+ `KHR_texture_transform` for baseColor)
  - Node index↔name map
- Docs: usage and format spec in `docs/tools/gltf-baker.md`.
- CI: golden tests (hash baked output for a small fixture).

## References
- docs/dossiers/bevy-gltf-vs-our-loading.md
- docs/dossiers/red-wyvern-loading.md
- docs/research/bevy.md
