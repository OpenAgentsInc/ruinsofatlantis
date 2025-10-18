# Model Viewer (tools/model-viewer)

This document explains how the standalone wgpu model viewer works end‑to‑end: file formats, loaders, animation merging/retargeting, UI, keyboard/mouse controls, and common debugging playbooks. It’s written for agents and contributors so issues can be diagnosed quickly.

Related
- Dossier: `docs/dossiers/red-wyvern-loading.md` — Red Wyvern loading, animations, textures, and integration steps.

## Purpose
- Inspect GLTF/GLB/FBX assets offline with the same CPU loaders used by the engine (`shared/assets`).
- Preview skinning, submesh materials, and animation clips.
- Merge animation libraries (GLB/GLTF/FBX) onto a base rig for quick validation.

## Binaries & Entrypoint
- Binary: `tools/model-viewer`
- Entry: `tools/model-viewer/src/main.rs`
- Shaders: `tools/model-viewer/src/shader_skinned.wgsl`, `tools/model-viewer/src/shader_basic.wgsl`

Run:
- `cargo run -p model-viewer -- <path-to-gltf-or-glb>`
- Snapshot: `cargo run -p model-viewer -- --snapshot dist/out.png <model>`

## Inputs and Flags
- `path` — base model (`.gltf` or `.glb`).
- `--anim-lib <path>` — additional library (`.gltf/.glb/.fbx`) to merge.
- `--wireframe` — enable wireframe (if supported).
- `--ui-scale <f>` — UI text scale (default `0.7`).
- `--head-pitch-deg <f>` — pitch correction for rigs with down‑tilted heads.

## Asset Search Locations
- Models list: scans `assets/models/**` (depth 4).
- Animations list: scans `assets/anims/**` (includes `fbx`, `gltf`, `glb`).
- Auto‑merge candidates (when no `--anim-lib`):
  - `assets/anims/converted/<stem>.glb`
  - `assets/anims/dragons/<stem>.(glb|gltf|fbx)`

## File Formats & Converters
- Preferred runtime formats: `.glb`/`.gltf` (no Draco at runtime).
- FBX support is behind a feature flag in `shared/assets`; default builds use best‑effort conversion:
  - `assimp export <file.fbx> <out.glb>` (installed during setup)
  - Output goes to `assets/anims/converted/` and is merged automatically.

## CPU Types (shared/assets)
- `SkinnedMeshCPU` contains:
  - `vertices` (pos/nrm/uv + joints/weights)
  - `indices`
  - `joints_nodes`, `inverse_bind`, `parent`, `base_t/r/s`, `node_names`
  - `submeshes` with optional `base_color_texture`
  - `animations: HashMap<String, AnimClip>`
- `AnimClip` includes T/R/S tracks keyed by node index in the base’s node array.

## Loading Flow
1. Parse GLTF/GLB via `roa_assets::skinning::load_gltf_skinned` or `load_gltf_mesh` (fallback).
2. Create GPU buffers: `VSkinned` vertex buffer, index buffer, and a storage buffer for the skin palette.
3. Build materials:
   - If `submeshes` have textures, upload them as SRGB `Rgba8UnormSrgb`.
   - If missing, fall back to a 1×1 white texture.
4. Compute bind‑pose palette on CPU and upload it.
5. Build `AnimData` for CPU sampling of palettes during playback.

## Animation Merging & Retargeting
- `merge_gltf_animations(base, lib_path)`
  - Loads the other rig; maps node names to the base using `normalize_bone_name` rules.
  - T/R/S tracks are retargeted into base space using rest‑pose deltas.
  - As of tools/dragon-viewer-polish, clips with zero mapped tracks are skipped.
- `merge_fbx_animations(base, fbx_path)`
  - Stub unless built with feature; we auto‑convert FBX→GLB using `assimp` and then call the GLTF merge.

### Clip Selection Policy in the Viewer
- After load or merge, the viewer lists only clips that affect the skinned joints (intersection with `joints_nodes`).
- Camera/object‑only clips are ignored.

## GPU Shaders (WGSL)
- Skinned pipeline: samples a single baseColor texture and writes to the swapchain.
- Basic pipeline (unskinned): constant grey albedo.
- Depth testing enabled; no lighting (unlit preview).

## UI & Controls
- Minimal panel in the top‑left (toggle with `Tab`).
- Lists (Models/Animations/Library) are hidden by default (toggle with `L`).
- Mouse:
  - Right‑drag: orbit; wheel: dolly.
- Keys:
  - `[` / `]` or Left/Right: previous/next animation.
  - `O`: autorotate toggle.
  - `H`: reset head pitch.
  - `Tab`: toggle whole overlay; `L`: toggle lists.

## Logging
- Use `RUST_LOG=info cargo run -p model-viewer -- <model>`.
- The viewer logs: loader decisions, merge counts, conversions, head‑pitch adjustments.

## Common Issues & Fixes
1. White model (no textures)
   - Expected if the asset lacks baseColor textures. Check `submeshes.len()` and whether `base_color_texture` is `None`.
   - For UDIMs or external textures, convert to GLB with embedded textures.
2. No animations playing
   - Ensure the listed clips affect skin joints. The viewer now filters to joint‑affecting clips; if the list is empty, your GLB has no bone tracks.
   - Provide an animation library GLB with skinned tracks and a compatible skeleton, or export the FBX animations to GLB (already automated in `assets/anims/converted`).
3. “CameraAction” or mesh‑only clips
   - These are filtered from the list if they don’t touch joints. Export bone animations from DCC or use a cleaned GLB.
4. Skeleton mismatch
   - Retargeting uses name‑based mapping. See `normalize_bone_name` in `shared/assets/src/skinning.rs` for prefixes and synonyms.
   - If bones don’t map, merge returns `0` and a warning is logged.

## Debug Checklist
- Inspect `skinned.node_names` and `skinned.joints_nodes` to confirm bone names.
- Print merged clip counts; verify at least one T/R/S track maps to a joint index.
- Use `--snapshot` to capture a frame and confirm geometry.
- Temporarily force a known working rig (e.g., UBC) to isolate pipeline issues.

## Known Limitations
- Single baseColor texture per submesh; no PBR shading.
- No Draco decoding at runtime; decompress ahead of time.
- FBX merging requires the optional feature or external conversion.

## Owners
- Tools: `tools/model-viewer` (Tools team)
- Asset loaders: `shared/assets` (Assets)
- Graphics: `crates/render_wgpu` (Graphics)
