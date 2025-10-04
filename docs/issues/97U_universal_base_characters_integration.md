# 97U — Integrate Universal Base Characters (Male/Female)

Status: PROPOSED

Labels: assets, characters, animation, renderer
Depends on: 95E1 (controls), 95F (voxel_upload), 95I/95K (replication), 95O (controller), existing skinned mesh pipeline

Intent
- Import and render the “Universal Base Characters” pack (male/female) as skinned characters with proper materials and textures, ready for runtime animation and controller use.

Source (local)
- Folder: `/Users/christopherdavid/Downloads/Universal Base Characters[Standard]/Base Characters`
  - Godot variant: `Godot/Superhero_Male.gltf + .bin + textures`, `Godot/Superhero_Female.gltf + .bin + textures`
  - Unreal variant: `Unreal Engine/Superhero_Male.gltf + .bin + textures`, `Unreal Engine/Superhero_Female.gltf + .bin + textures`
  - Unity variant: `.fbx` (skip; prefer glTF/GLB for our loader)

Plan
- Place assets under `assets/models/ubc/` preserving relative texture paths. Prefer the “Unreal Engine” or “Godot” `.gltf + .bin` variants for compatibility with `ra_assets::gltf`.
- Load both meshes via existing `ra_assets::gltf` helpers; extend loaders to fetch skin/joint data and material maps if needed.
- Ensure texture references resolve (our gltf loader uses `import` and relative paths). Keep the directory structure intact.
- Drive animations once `97A` (Animation Library) lands; for now, validate bind pose + materials.

Files to touch
- `shared/assets` (crate `ra-assets`):
  - Add a small loader to extract: skinned vertex streams (pos/norm/uv), joint indices/weights, and materials/textures into `SkinnedMeshCPU` and a `MaterialCPU` with PBR maps (basecolor/normal/roughness).
- `crates/render_wgpu/src/gfx/material.rs` and `pipeline.rs`:
  - Confirm PBR material binding supports basecolor/normal/roughness maps; add hair/eye textures as needed.
- `crates/render_wgpu/src/gfx/scene.rs`:
  - Add a demo scene variant spawning one male + one female UBC model (feature-gated or a small toggle) to validate materials and joint palettes.
- `crates/render_wgpu/src/gfx/anim.rs`:
  - Ensure palette builds for arbitrary joint counts from glTF skin; allocate palettes buffer sized to max joints across active skinned sets.

Tasks
- [ ] Copy `.gltf/.bin` + textures under `assets/models/ubc/{male, female}/` (preserve filenames exactly).
- [ ] Extend `ra_assets::gltf` loader to parse skins and materials (if our current mesh loader is “mesh-only”).
- [ ] Build `SkinnedMeshCPU` for UBC and verify joints count; expose via renderer init.
- [ ] Create a demo spawn: place UBC male/female at fixed points; verify materials render and joint palettes upload.
- [ ] Wire simple idle animation once `97A` is integrated (see that issue).
- [ ] Track assets in Git LFS (large binaries) per repo policy.

Acceptance
- Both UBC male and female load and render with correct materials (basecolor + normals + roughness). No missing textures.
- Joint palettes allocate correctly (no over/under-indexing); renderer supports per-model joint count.
- Assets resolve via relative paths when the app is run from repo root.

Notes
- File size / LFS: add `*.gltf`, `*.bin`, and textures to LFS if they exceed thresholds; preserve vendor folder structure.
- License: include `License_Standard.txt` under `docs/third_party/` or append to `NOTICE` per policy; keep original filename.

