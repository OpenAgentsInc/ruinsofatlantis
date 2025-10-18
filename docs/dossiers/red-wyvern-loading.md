# Red Wyvern Loading — Comprehensive Dossier

This is the canonical, all‑in‑one reference for the Red Wyvern asset: where files live, how the loaders work, how the viewer renders it, how the game renderer wires it today, and what remains to enable full animations in‑game.

## Quick Facts
- Base model: `assets/models/red_wyvern/RedDragon2021.glb`
- Packed model (no Meshopt/Draco, embedded images): `assets/models/red_wyvern/RedDragon2021.textured.glb`
- Anim lib: `assets/anims/converted/RedDragon2021.glb` (source FBX also present)
- Export script: `scripts/blender/export_glb_clean.py`
- Shared loaders: `shared/assets/src/*`
- Viewer: `tools/model-viewer/**`
- Renderer glue: `crates/render_wgpu/src/gfx/**`

## Current State
1) Viewer (tools/model-viewer)
- Loads the Wyvern via `roa_assets::skinning::load_gltf_skinned`, can merge compatible animation libraries, and displays textured submeshes. Orientation toggle commonly set to -90° around X for this rig. Shader: simple lambert with SRGB baseColor.

2) Game renderer (crates/render_wgpu)
- A textured static fallback is implemented and visible in cc_demo. It aggregates all primitives, uploads a VB/IB, installs a material BG from the per‑primitive baseColor, and draws via the textured instanced pipeline.
- The skinned path attempts to load the rig but currently reports `skinned idx=0 joints=0`, so the draw falls back to the static path.

Bottom line: the dragon renders (static), but animations are not active in‑game yet because the engine isn’t receiving a skinned, jointed Wyvern GLB with usable clips.

## File & Module Map (clickable)
- Shared loaders and types
  - shared/assets/src/skinning.rs
  - shared/assets/src/retarget.rs
  - shared/assets/src/draco.rs
  - shared/assets/src/util.rs
  - shared/assets/src/types.rs
- Viewer
  - tools/model-viewer/src/main.rs
  - tools/model-viewer/src/shader_skinned.wgsl
- Renderer (engine)
  - crates/render_wgpu/src/gfx/wyvern.rs
  - crates/render_wgpu/src/gfx/draw.rs
  - crates/render_wgpu/src/gfx/renderer/init.rs
  - crates/render_wgpu/src/gfx/renderer/passes.rs
  - crates/render_wgpu/src/gfx/material.rs
  - crates/render_wgpu/src/gfx/pipeline.rs
  - crates/render_wgpu/src/gfx/shader.wgsl

## Asset Prep (what we did and why)
The original GLB contained Meshopt compression and external images in some variants; the engine’s plain reader returned empty attributes, causing a DEBUG cube. We generated a “raw” GLB with embedded textures and no Meshopt/Draco:

```bash
npx -y gltfpack -i assets/models/red_wyvern/RedDragon2021.glb \
  -o assets/models/red_wyvern/RedDragon2021.textured.glb -noq
```

The engine now reads vertex/index data for the static fallback reliably and uploads a proper SRGB baseColor.

## Loader Behavior (shared/assets)
- Skinned load (shared/assets/src/skinning.rs)
  - Picks the dominant skin by vertex count, aggregates all skinned primitives referencing that skin, extracts per‑primitive baseColor images, and returns `SkinnedMeshCPU` plus a clip list.
  - `merge_gltf_animations` can merge a Wyvern anim library; it retargets by name, skipping unmapped clips.

- Static GLB load (shared/assets/src/gltf.rs)
  - Merges meshes/primitives into a CPU mesh of `Vertex` and `u16` indices; decodes Draco JSON when needed for wasm.

## What the Renderer Does Today
- Init: crates/render_wgpu/src/gfx/renderer/init.rs
  - Loads skinned Wyvern CPU data; if `index_count==0 || joints==0`, builds a static fallback VB/IB and an optional material BG.
  - Logs: `wyvern: assets summary — skinned idx=… joints=… | static idx=… used=…`.

- Static renderer path (textured)
  - Build VB (VertexPosNrmUv) + IB and an SRGB 2D texture for the baseColor if present.
  - Draw via instanced textured pipeline; material BG bound at set=3.
  - Code: crates/render_wgpu/src/gfx/wyvern.rs (load), crates/render_wgpu/src/gfx/draw.rs (draw_wyvern_static), crates/render_wgpu/src/gfx/pipeline.rs (create_textured_inst_pipeline).

- Orientation
  - We apply conservative Rx(-90°) first; optional roll/yaw tweaks are composed in `wyvern_model_m`. See init.rs for the current composition and log history when adjusting.

## EXACT TODOs to Enable Animations In‑Game
1) Export a skinned Wyvern GLB with embedded textures + NLA clips
- Use the pipeline below to produce `assets/models/red_wyvern/RedDragon2021.textured.glb` with JOINTS/WEIGHTS and pushed actions:

```bash
BLENDER="/Applications/Blender.app/Contents/MacOS/Blender"
IN="$HOME/Desktop/RedWyvern/uploads_files_2877852_FireBreathingWyvernDragon(update).blend"
OUT="assets/models/red_wyvern/RedDragon2021.textured.glb"
IMAGES_DIR="assets/models/red_wyvern/udims"

"$BLENDER" -b "$IN" --python scripts/blender/export_glb_clean.py -- \
  --in "$IN" --out "$OUT" \
  --strip-cams --strip-lights --strip-empties \
  --pack --push-actions --images-dir "$IMAGES_DIR"
```

Acceptance: `SkinnedMeshCPU` from `load_gltf_skinned(OUT)` has `joints_nodes.len()>0` and `indices.len()>0` and the viewer lists ≥1 clips.

2) Optional: Meshopt decoding for skinned attributes
- If future exports contain `EXT_meshopt_compression`, add decoding in the skinned reader (similar to the planned static Meshopt‑aware routine) so positions/indices/joints/weights are readable without re‑exporting.

3) Upload and render via the existing wizard skinned pipeline
- Build `VertexSkinned` VB + IB, create a storage buffer for joint palettes, and per‑submesh material BGs (SRGB).
- Wire binds identical to the wizard/zombie rigs and call the skinned pass.

4) Animate each frame
- Port the viewer’s palette sampler (`AnimData`) or integrate an animator that writes palette matrices per frame for the active clip:

```rust
let palette = anim.sample_palette(active_clip, time);
queue.write_buffer(&wyvern_palettes_buf, 0, bytemuck::cast_slice(&palette));
```

5) Keep logs + CI guardrail
- When `skinned idx>0` and `joints>0`, log once and run a minimal CI check (load → sample → draw) to guard regressions.

## Troubleshooting Matrix (engine)
- Cube renders / model invisible → asset uses Meshopt/Draco and static path lacked decode. Use `*.textured.glb` (gltfpack -noq) or add Meshopt decode.
- Textures white → no `baseColorTexture` for that primitive or UDIM tiles not embedded. Re‑export with `--pack` or bake UDIMs to 0–1.
- Skinned path idx=0/joints=0 → export lacks skin or the wrong GLB is referenced; generate a proper skinned GLB as above.

## Commands
- Generate raw packed GLB:

```bash
npx -y gltfpack -i assets/models/red_wyvern/RedDragon2021.glb \
  -o assets/models/red_wyvern/RedDragon2021.textured.glb -noq
```

- Viewer with logs:

```bash
RUST_LOG=info,roa_assets=info cargo run -p model-viewer -- \
  assets/models/red_wyvern/RedDragon2021.textured.glb
```

- Game with wyvern logs (cc_demo):

```bash
ROA_ZONE=cc_demo \
RUST_LOG=info,wyvern=info,render_wgpu::gfx::renderer::init=info,render_wgpu::gfx::renderer::passes=info \
cargo run
```

## Sources & Pointers
- Assets
  - assets/models/red_wyvern/RedDragon2021.glb
  - assets/models/red_wyvern/RedDragon2021.textured.glb
  - assets/models/red_wyvern/udims/Dragon Skin.1001.png
  - assets/anims/dragons/RedDragon2021.fbx
  - assets/anims/converted/RedDragon2021.glb
- Shared loaders
  - shared/assets/src/skinning.rs
  - shared/assets/src/retarget.rs
  - shared/assets/src/draco.rs
  - shared/assets/src/util.rs
  - shared/assets/src/types.rs
- Viewer
  - tools/model-viewer/src/main.rs
  - tools/model-viewer/src/shader_skinned.wgsl
- Renderer
  - crates/render_wgpu/src/gfx/wyvern.rs
  - crates/render_wgpu/src/gfx/draw.rs
  - crates/render_wgpu/src/gfx/renderer/init.rs
  - crates/render_wgpu/src/gfx/renderer/passes.rs
  - crates/render_wgpu/src/gfx/material.rs
  - crates/render_wgpu/src/gfx/pipeline.rs
- Docs
  - docs/graphics/model-viewer.md
  - docs/gdd/11-technical/graphics/model-loading.md

