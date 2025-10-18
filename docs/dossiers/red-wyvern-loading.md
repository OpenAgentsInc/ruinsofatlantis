# Red Wyvern Loading — Comprehensive Dossier

Summary
- Goal: Document how the Red Wyvern (assets/models/red_wyvern/RedDragon2021.glb) is loaded, animated, and textured in the model viewer, current limitations (animations, UDIM textures), and the path to integrate it into the main scene.
- Audience: New contributors and agents who need full context to debug or extend dragon loading and bring it into the game scene.

Scope
- Covers loaders in `shared/assets`, viewer specifics in `tools/model-viewer`, relevant assets and scripts under `assets/**` and `scripts/**`, and history/issues.
- Out of scope: Final engine integration draw code (renderer) implementation; this dossier provides concrete integration steps and references.

Assets Inventory
- Base model
  - `assets/models/red_wyvern/RedDragon2021.glb`
  - UDIM textures present (external files): `assets/models/red_wyvern/udims/Dragon Skin.100{1..4}.png`, `assets/models/red_wyvern/udims/Dragon Subsurf.100{1..4}.png`
- Animation libraries
  - Raw FBX: `assets/anims/dragons/RedDragon2021.fbx`
  - Converted GLB: `assets/anims/converted/RedDragon2021.glb`
- Blender export script (headless)
  - `scripts/blender/export_glb_clean.py`

Key Code Paths
- Viewer entry and skinned pipeline
  - `tools/model-viewer/src/main.rs:718` — material bind group layout (`mat_bgl`).
  - `tools/model-viewer/src/main.rs:980` — build per-submesh material bind groups, upload SRGB textures.
  - `tools/model-viewer/src/main.rs:1120` — build `AnimData`, choose default clip (longest), handle head-pitch correction.
  - `tools/model-viewer/src/main.rs:1430` — auto-merge animations by filename stem from `assets/anims/**` (dragons/converted).
  - `tools/model-viewer/src/shader_skinned.wgsl:1` — skinned pipeline; baseColor sampling + simple lambert for shape readability.
- CPU asset loading and animations
  - `shared/assets/src/skinning.rs:14` — `load_gltf_skinned(path)`: dominant-skin selection, multi-node aggregation, Draco decode for skinned prims, per-primitive baseColor extraction into `SubmeshCPU`.
  - `shared/assets/src/skinning.rs:513` — `merge_gltf_animations(base, lib_path)`: clip retarget by node-name mapping; skips clips with zero mapped tracks; logs mapped T/R/S counts per clip.
  - `shared/assets/src/retarget.rs` — rotation retarget math and helpers for mapping name-equivalent bones.
  - `shared/assets/src/draco.rs:18` — `decode_draco_skinned_primitive` reads `KHR_draco_mesh_compression` and expands POSITION/NORMAL/UV/JOINTS/WEIGHTS.
  - Types: `shared/assets/src/types.rs:53` (`TextureCPU`), `:61` (`SubmeshCPU`), `:77` (`SkinnedMeshCPU`).
  - Path resolver: `shared/assets/src/util.rs:6` (`prepare_gltf_path`) — prefers decompressed/packed alternates.
- Graphics overview and viewer docs
  - `docs/graphics/model-viewer.md:1` — model viewer architecture and troubleshooting.
  - `docs/gdd/11-technical/graphics/model-loading.md:1` — GLTF loading behavior (dominant skin, submeshes, Draco, textures).

Current Behavior — Red Wyvern in the Viewer
- Loading flow
  - The viewer scans `assets/models/**` and lists `RedDragon2021.glb` (and any `*.textured.glb` variants). Drag-and-drop also works.
  - On load, `roa_assets::skinning::load_gltf_skinned` selects the dominant skin by vertex count and aggregates all skinned primitives across nodes referencing that skin.
  - For each primitive (material), if `pbr_metallic_roughness.base_color_texture` exists, the image is uploaded into a GPU SRGB texture; a per-submesh bind group is created.
  - The viewer builds a CPU palette sampler (`AnimData`) and plays the longest clip by default; head-pitch upright correction can be applied via `--head-pitch-deg`.
- Animation merging
  - If no explicit `--anim-lib` is passed, the viewer auto-searches for `<stem>.(glb|gltf|fbx)` under `assets/anims/converted`, `assets/anims/dragons`, then `assets/anims` and merges any found clips into the loaded rig.
  - `merge_gltf_animations` maps node names via `normalize_bone_name` and retargets rotations using rest-pose deltas. Clips with zero mapped tracks are skipped with a warning.
  - FBX is converted to GLB via `assimp` (if available) or via `fbx2gltf` when present; results are cached under `assets/anims/converted/`.
- Model orientation
  - The viewer includes a model-rotation toggle; for Red Wyvern we commonly use “-90° X” to orient into the viewer’s +Y-up.

Textures & UDIMs
- What works today
  - For each submesh, if the GLB references a single baseColor texture, the viewer uploads it and shades with a simple lambert. This yields correct color for packed/embedded textures.
- UDIM limitation
  - GLTF exporters often express UDIMs as multiple images per material or via non-standard conventions; our loader selects a single `base_color_texture` per primitive.
  - If the Red Wyvern GLB references UDIM tiles externally, you can get white or flat color on some surfaces.
- Recommended fixes
  - Prefer a “packed” GLB with embedded textures for viewer/engine: use `scripts/blender/export_glb_clean.py` with `--pack` to embed.
  - If UDIMs are required, bake them to a single 0–1 albedo per material in Blender and export; or implement a future path to detect/resolve UDIM tiles into a single texture atlas per submesh.
  - Validate by checking logs for `viewer: material #i baseColor WxH` (non-1×1) and `viewer: submeshes=N (with textures=M)` with `M>0`.

Common Symptoms and Root Causes
- “White model” or flat grey
  - Cause: no `baseColor` image for that submesh, or GLB references UDIMs not embedded; we fall back to 1×1 white.
  - Source: `tools/model-viewer/src/main.rs:1009` (white fallback), `shared/assets/src/skinning.rs:290+` (per-primitive texture load).
- “No animations listed/playing”
  - Cause: base GLB has camera/object tracks only, or merged library clips don’t map to the base rig.
  - Fix: merge `assets/anims/converted/RedDragon2021.glb` (or the FBX via converter). The viewer filters to joint-affecting clips; if empty, it falls back to all clip names for manual testing.
- “Wrong orientation”
  - Use the viewer’s model-rotation toggle; Red Wyvern typically needs -90° X.

How-To: Reproduce and Verify Locally
- Load with logs
  - `RUST_LOG=info,roa_assets=info cargo run -p model-viewer -- assets/models/red_wyvern/RedDragon2021.glb`
  - Expect: messages selecting a skin, appending primitives, and printing per-submesh baseColor dimensions.
- Merge animations (automatic)
  - Name stems match; the viewer finds `assets/anims/converted/RedDragon2021.glb` and merges mapped clips. The animation list shows joint-affecting clips; longest is active.
- Troubleshoot textures
  - If some submeshes are white: ensure GLB has embedded baseColor images (see export script); otherwise bake UDIMs to 0–1.

Integration Plan — Add Red Wyvern to Main Scene
1) CPU asset load
   - Use `roa_assets::skinning::load_gltf_skinned("assets/models/red_wyvern/RedDragon2021.glb")` to get `SkinnedMeshCPU`.
   - Validate `submeshes` and `base_color_texture` presence; prefer “packed” GLB.
2) GPU resources (mirror wizard pipeline)
   - Vertex/index buffers: same `VertexSkinned` layout as wizards (`crates/render_wgpu/src/gfx/types.rs:53`).
   - Skin palette storage buffer; bind group matches wizard pipeline.
   - Material bind group per submesh (sampler + SRGB 2D texture). See viewer path for a minimal template.
3) Draw integration
   - Either: reuse the wizard skinned pipeline (`crates/render_wgpu/src/gfx/shader.wgsl:378`) with per-instance model matrices.
   - Or: add a simple unlit-lambert variant if you want exact viewer parity quickly.
   - Render order: opaque first; if any parts need alpha, treat them as masked/transparent later.
4) Animation playback
   - Sample palettes on CPU (like viewer’s `AnimData`) or integrate with existing animator that drives wizards/zombies; seed a single instance with an idle/fly clip.
   - For merged dragon library clips, keep the name-based mapping; ensure joint names are consistent after export.
5) Placement
   - Start with one instance near the ruins; add a simple orbit/fly loop to validate culling and animation.

History (Commits)
- dcb866f… — tools: load Red Wyvern model, auto-merge dragon anims, and compact UI (toggle lists) (.git/logs/HEAD:1653)
- fc85cc6… — viewer: add GPU model rotation (default -90x for Red Wyvern), lambert shading; add Blender bake script (.git/logs/HEAD:1673)
- b884d74… — assets: skip empty retargets when merging; viewer: auto-swap removed later, keep base textures (.git/logs/HEAD:1655/1659)
- fca3900… — viewer: filter clips to joint-affecting; neutral grey fallback; docs/model-viewer (.git/logs/HEAD:1656)

Related Issue
- #119 — Blender headless export script: clean GLB with textures + clips (start with Red Wyvern)

Open Risks and Follow-Ups
- UDIM handling: bake to 0–1 or implement multi-tile resolve.
- PBR materials: expand to normal/ORM textures (viewer and engine) to match expected look.
- Engine animator: consolidate CPU palette sampling utilities so viewer and renderer share code.

Validation Checklist
- Viewer lists joint-affecting clips for Red Wyvern and plays the longest by default.
- Submeshes report non-1×1 baseColor dimensions; M>0 textured submeshes.
- Engine integration draws at least one animated instance with expected orientation and color.

Sources
- Code
  - tools/model-viewer/src/main.rs:980
  - tools/model-viewer/src/shader_skinned.wgsl:1
  - shared/assets/src/skinning.rs:14
  - shared/assets/src/draco.rs:18
  - shared/assets/src/types.rs:53
  - shared/assets/src/util.rs:6
- Assets
  - assets/models/red_wyvern/RedDragon2021.glb
  - assets/models/red_wyvern/udims/Dragon Skin.1001.png
  - assets/anims/dragons/RedDragon2021.fbx
  - assets/anims/converted/RedDragon2021.glb
- Docs
  - docs/graphics/model-viewer.md:1
  - docs/gdd/11-technical/graphics/model-loading.md:1
- Issues/Commits
  - #119 (Blender export)
  - dcb866f… (tools: load Red Wyvern)
  - fc85cc6… (viewer: rotate/model shading)

