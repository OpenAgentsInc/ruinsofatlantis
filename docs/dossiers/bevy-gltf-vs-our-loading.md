# Bevy `bevy_gltf` vs. Ruins of Atlantis GLTF/Skinned Loading

This dossier explains how Bevy’s `bevy_gltf` crate loads and represents glTF assets end‑to‑end and contrasts that with our current approach in Ruins of Atlantis (RoA). It focuses on structure, data flow, animation/skin handling, materials, configuration, and extension coverage — with practical implications for stability and parity.

The goal is to make it clear where Bevy’s defaults “just work,” where they differ from our customized path, and which ideas we may want to adopt to harden our pipeline.

## Executive Summary

- Bevy integrates glTF deeply with ECS: a glTF becomes a graph of Entities (Scene), with Assets for meshes/materials/skins/animations, plus components (e.g., `SkinnedMesh`).
- Bevy’s loader is configuration‑driven (loader settings, plugin defaults) and extension‑aware for PBR.
- Our pipeline is lower‑level and renderer‑specific: we decode a single skinned mesh into custom CPU types, manually choose pipeline/bindings, and update palettes per frame. We merge external clips by name mapping and offered an override env for fast iteration.
- Bevy prioritizes correctness and generality across features; we optimize for “demo control” (explicit candidates, env overrides, custom shaders).

Implication: Bevy’s structure reduces whole classes of binding/layout errors by construction (consistent BGLs, PBR material graph, ECS‑driven Scene). Our custom path allows tight control but requires meticulous binding/format discipline and more diagnostics.

## Architecture Overview

### Bevy `bevy_gltf`

- Plugin (`GltfPlugin`) registers an `AssetLoader` for `.gltf`/`.glb` and exposes a typed `Gltf` asset that references child assets by handles (meshes, materials, skins, nodes, scenes, animations).
- A glTF Scene becomes a Bevy `Scene` (a `World`) populated with Entities/Components. Nodes are Entities; meshes attach `Mesh3d`/materials; skinning attaches `SkinnedMesh` with joints and inverse bind matrices; optional `AnimationPlayer` is inserted when the feature is enabled.
- Materials map to `StandardMaterial` (PBR) with extensive KHR extension support, sampler controls, and sRGB/linear handling.
- Images/Textures are asynchronously loaded (parallel on native), then materials are resolved from textures.
- Coordinate conversion is configurable (global switch or per‑load setting), impacting forward direction for cameras/lights/models.
- Loader settings (per load) control what to load (meshes/materials/cameras/lights/animations), VRAM residency (`RenderAssetUsages`), default sampler overrides, and whether to include the glTF source.

Key traits:
- ECS‑first: assets + scene graph become Entities and Components.
- PBR‑first: materials and textures flow into Bevy’s PBR pipeline.
- Declarative: labels (“#Scene0”, etc.) and settings drive what is instantiated.
- Robustness: well‑formed binding layouts and material pipelines are part of the engine contract.

### Ruins of Atlantis (current)

- We decode a targeted skinned mesh into custom CPU structs (vertices, joints, weights, inverse binds, per‑submesh optional baseColor) and merge animation clips from companion GLBs by name mapping.
- Rendering uses renderer‑specific pipelines: wizard (shared) or a wyvern‑only shader/pipeline path. We upload skin palettes each frame and bind materials per submesh (when present). We added a bind‑pose freeze for troubleshooting.
- Asset selection is explicit and override‑friendly: we honor `ROA_WYVERN_BASE` (and `OA_WYVERN_BASE`) and probe a short candidate list. We intentionally do not deep‑scan the tree to reduce ambiguity.
- Materials: simple baseColor handling; when skinned GLB lacks textures we render untextured (white) to avoid UV smearing from unrelated static assets.
- Diagnostics: verbose logs for probing, clip merges; recently added a viewer‑parity wyvern pipeline to remove binding ambiguity.

Key traits:
- Renderer‑first: custom buffers, custom bind group orders, custom shaders.
- “Demo control”: env overrides, minimal candidates, explicit merges.
- Flexible but fragile: requires careful attribute layouts and binding order; missing guardrails can surface as “shredded” meshes.

## Data Model & Scene Graph

- Bevy
  - `Gltf` asset aggregates handles to `GltfMesh`, `GltfNode`, `GltfSkin`, `StandardMaterial`, `AnimationClip`, etc.
  - Scenes become a `Scene` (Bevy `World`) with Entities; each glTF node is an Entity with `Transform`, optional mesh/material, optional `SkinnedMesh` with `joints: Vec<Entity>` and `inverse_bindposes`.
  - Benefits: coherent scene hierarchy, easy traversal/query, built‑in systems update skinning uniforms, animation drives Entities by name/path.

- RoA
  - No import to ECS scene today; we keep a single skinned mesh + palette and draw via renderer code paths, with a lone instance buffer for transform/color/selection.
  - Benefits: simplicity in a controlled demo; fast “point at this GLB and draw.”
  - Tradeoff: mismatch risk increases (attribute layouts, bind orders, per‑submesh indexing), and we miss ECS‑level safety nets.

## Skinning & Palettes

- Bevy
  - `SkinnedMesh` component holds joints (Entities) and inverse bind matrices; render systems prepare the skin palette for the shader. Joints come from the node graph; order is consistent with the skin.
  - `MAX_JOINTS` guard; logs warnings when exceeded.

- RoA
  - We compute the palette on CPU each frame from sampled clips and upload to a storage buffer. We added guard rails (clamp joint indices, renormalize weights).
  - We recently corrected vertex attribute order and introduced a wyvern‑only pipeline with stable group order to remove binding ambiguity.

Insights:
- Bevy’s ECS joint mapping and consistent palette update path help avoid palette order/offset bugs. We must ensure our joints order and inverse bind indices match glTF skin order 1:1 and validate lengths/offsets at draw.

## Animations

- Bevy
  - With the `bevy_animation` feature, Bevy builds `AnimationClip`s from glTF animations, adds `AnimationPlayer` to animation roots, and targets nodes by name/path (`AnimationTargetId`). Playback is a runtime system concern.

- RoA
  - We load glTF clips and also merge from sidecar GLBs by name mapping (string normalizations). We select a clip heuristically (Idle/Fly_Loop/longest), with an optional bind‑pose freeze.

Implications:
- Bevy’s name/path mapping is robust across files because it ties to Entities; our string‑based name normalization is pragmatic but brittle. Adding a “preferred clip” env and logging target mappings will reduce surprise.

## Materials & Textures

- Bevy
  - Comprehensive PBR: `StandardMaterial` fields are populated from glTF baseColor/metallicRoughness/normal/occlusion/emissive and supported KHR extensions (anisotropy, clearcoat, specular, transmission, volume, unlit, emissive strength…).
  - Controlled sRGB vs linear sampling; samplers may be overridden; compressed formats supported based on feature set and backend.

- RoA
  - Simple baseColor only (no full PBR). If the skinned GLB lacks textures, we render untextured (white) by design.
  - We pre‑bake submesh BGs; previously tried to “borrow” textures from a static GLB, which produced UV mismatches — removed.

## Extensions & Format Support

- Bevy
  - advertises per‑extension support; draco/meshopt currently marked unsupported; KTX2/WebP supported as formats but not via glTF extension syntax (as of the referenced code version).

- RoA
  - Draco handled externally by preprocessing tools; we never decode Draco at runtime.

## Coordinate Systems & Transforms

- Bevy
  - Loader supports a “use_model_forward_direction” switch that flips global +Z/‑Z for models/cameras/lights on import, providing a consistent world convention.

- RoA
  - We apply a single Rx(‑90°) at the model instance to match exporter orientation. No ECS node graph to handle per‑node coordinate flips.

## Asset Settings & Residency

- Bevy
  - `GltfLoaderSettings` controls which parts load and whether assets are retained in MAIN/RENDER worlds.

- RoA
  - No residency management; we keep explicit buffers in renderer.

## Stability & Observed Failure Modes

- Bevy’s design prevents many low‑level issues:
  - Bind‑group layouts are created once and reused; PBR/material textures are wired consistently; ECS joint ordering comes directly from the glTF skin; animation targets derive from the node graph.

- Our historical problems tied to manual wiring:
  - Wrong vertex attribute order (fixed).
  - Swapped/set bind groups on certain pipelines (fixed for wyvern/static; established wyvern‑only shader).
  - Attempting to reuse textures from another GLB (removed).
  - Remaining deformation suggests a palette/joints order or transform space mismatch not yet caught by assertions.

## Practical Guidance — What We Can Borrow from Bevy

1) Assert and log palette invariants at draw time
- Validate palette length = joints × instances; dump first N matrices and first N joint indices/weights for one frame in a debug mode.

2) Tie animation targets to node indices, not just names
- Preserve a node index → name map and use that to merge clips, mirroring Bevy’s path targeting logic.

3) Treat GLB textures as part of the same asset only
- Never borrow baseColor from unrelated GLBs (already removed). If the skinned GLB has no textures, render untextured or apply a procedural preview (tri-planar) — but don’t mix assets.

4) Prefer a single, well‑documented pipeline
- Keep wyvern‑only shader/pipeline for parity and document group orders; avoid mixing multiple skinned pipelines. Bevy’s uniformity is a good model.

5) Consider a minimal ECS wrapper for the wyvern
- Represent the wyvern’s joints as a tiny node graph to leverage stable skin ordering and future animation retargeting.

## Closing Notes

Bevy’s `bevy_gltf` is engineered for generality and correctness across glTF features. Our pipeline favors targeted control for a live renderer demo. The remaining gap (mangling) almost certainly lives in joint/palette alignment or transform space propagation. Borrowing Bevy’s validation style and node/animation mapping will close that gap, while the dedicated wyvern pipeline ensures bind layouts stay simple and predictable.

This dossier is code‑free by intent; use the references above to navigate the exact files and systems in each codebase.

