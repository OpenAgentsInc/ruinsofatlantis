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


Animation export (DTO)
- --anims-out <path> writes AnimClip JSON with node index–based tracks:
  - name, duration
  - t_tracks (node, times, values[xyz])
  - r_tracks (node, times, values[xyzw])
  - s_tracks (node, times, values[xyz])

Examples
```
# Export skinned DTO
cargo run -p gltf-baker -- assets/anims/converted/RedDragon2021.glb --skinned-out out/wyvern.skinned.json

# Export animation DTOs
cargo run -p gltf-baker -- assets/anims/converted/RedDragon2021.glb --anims-out out/wyvern.anims.json
```


Materials v1 (baseColor)
- The Skinned DTO includes `submeshes[]` with start/count and optional baseColor texture (RGBA8, base64 encoded). KHR_texture_transform is not yet exported; v2 will add `uv_transform` per baseColor (offset, scale, rot).


Materials v2 (KHR_texture_transform)
- Set env `GLTF_BAKER_SRC=<input.glb>` to enable best-effort UV transform export.
- The skinned DTO `submeshes[]` includes `uv_transform` when available from `KHR_texture_transform`:
  - offset: [u,v], scale: [u,v], rot (radians)
- Note: Mapping assumes submesh order matches GLTF primitive order for the dominant skin (by vertex count).
