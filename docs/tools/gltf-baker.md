# gltf-baker (MVP)

Minimal, standalone utility to inspect and (eventually) bake GLTF/GLB assets into RoA-native formats.

Current status (MVP)
- Reads a `.gltf` or `.glb` and emits a JSON summary:
  - counts for scenes, nodes, meshes, skins, animations, materials
  - flags `has_draco` (fails if present — pre-decompress required)

Usage
```
cargo run -p gltf-baker -- assets/anims/converted/RedDragon2021.glb out/red_wyvern.summary.json
# or print to stdout
cargo run -p gltf-baker -- assets/anims/converted/RedDragon2021.glb
```

Next steps
- Switch to Bevy `bevy_gltf` backend and export RoA-native pack (SkinnedMeshCPU, AnimClip, submeshes, baseColor, node index↔name map).
- Add CLI options for selecting scenes, skipping materials, and bind-pose only.
- Integrate into `xtask` (e.g., `cargo xtask bake-gltf`).

Notes
- Draco is an offline preprocess: use our existing `gltf-decompress` tool and re-run `gltf-baker`.
