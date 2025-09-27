# Ruins of Atlantis — Lighting Roadmap (engine‑agnostic)

This document outlines a practical, staged plan to bring high‑quality, dynamic environment lighting to our Rust/wgpu renderer. It builds on our existing sky/sun/time‑of‑day system and charts a path toward robust global illumination and reflections without depending on any vendor‑specific features or naming.

Goals
- Strong outdoor lighting first: sun + sky drive the look across time‑of‑day (TOD).
- Stable, temporally‑accumulated indirect light and reflections with graceful fallbacks.
- Works on a wide range of GPUs; advanced paths are optional quality toggles.
- Clear module boundaries under `src/gfx/` with reusable temporal/scheduling utilities.

Current state (what we already have)
- `src/gfx/sky.rs` maintains Hosek–Wilkie sky on CPU with TOD and sun direction.
- `Globals` includes sun direction, SH‑L2 sky irradiance, and fog params for shaders.
- Forward pipelines for terrain, wizards, particles, and UI.

Overview of phases
1) Foundations: G‑Buffer, depth pyramid, temporal reprojection, SSR/SSGI baseline.
2) Indirect capture atlas (tile‑based surface captures) + sampling at ray hits.
3) Software ray tracing for off‑screen hits (triangle BVH; optional mesh SDFs).
4) Indirect radiance accumulation and multi‑bounce propagation + denoisers.
5) Reflection polish, translucency options, and quality switches.
6) Tools: overlays, inspectors, and perf budgets.

—

Phase 0 — Environment lighting baseline (connect to TOD)
- Directional sunlight: keep driving direct lighting from `Globals.sun_dir_time`.
- Sky ambient: continue using SH‑L2 from `sky.rs`, refreshed when TOD or weather changes.
- Energy consistency: ensure BRDF uses the same basis for direct and indirect terms (linear space, consistent roughness/metalness workflows).
- Validation: build a simple TOD sweep scene and lock exposure to catch color balance errors.

Phase 1 — Foundations
1. G‑Buffer v1 (deferred data)
   - Targets: world‑space normal, base‑color, roughness, metalness, emissive, depth, motion vectors.
   - Implementation sketch:
     - Add `src/gfx/gbuffer.rs` to create MRT render targets and resolve paths.
     - Extend `src/gfx/pipeline.rs` to build a G‑Buffer pass pipeline and layouts.
     - Add `gbuffer.wgsl` (vs/fs) or extend `shader.wgsl` with entry points for G‑Buffer output.
     - For current meshes (terrain, wizards), author minimal material params to populate the buffers.
   - Notes: pack normals in 2‑components if bandwidth constrained; keep albedo emissive in linear HDR format.

2. Temporal reprojection core
   - Add `src/gfx/temporal/` with `reprojection.rs` and `history.rs` to share across GI and reflections.
   - Inputs: motion vectors, depth, previous view‑proj, history textures.
   - Techniques: neighborhood clamping and reactive masks to suppress ghosting on emissive/flicker.

3. Depth pyramid (Hi‑Z)
   - Add `src/gfx/hiz.rs` + compute shader for mip‑down depth. Used by SSR/SSGI ray marching and denoisers.
   - Build/update once per frame after the G‑Buffer pass.

4. SSR/SSGI baseline
   - Screen‑space reflections: importance sample reflection direction, march in Hi‑Z, binary search refine, temporal accumulate; fall back to environment map when miss.
   - Screen‑space diffuse GI: cosine‑weighted screen‑space rays with short march budget, temporal accumulate; fall back to sky SH when miss.
   - Add `src/gfx/reflections/ssr.rs` and `src/gfx/gi/ssgi.rs` with matching WGSL entry points.

Phase 2 — Tile‑based indirect capture atlas (our “surface capture” analog)
Goal: When rays leave the screen, fetch pre‑captured material/lighting from nearby geometry.

1. Mesh capture descriptors (offline/at import)
   - For each mesh, generate a small set of capture views that cover its surfaces (hemisphere directions biased to the normal distribution of the asset).
   - Store per‑view parameterization to map a hit point to a 2D capture domain (UV‑free, card‑like but named generically here as “capture tiles”).
   - Persist a compact descriptor per mesh: tile count, view directions, projection parameters, and texture footprint hints.
   - Add tool hooks under `tools/` or in `assets/gltf.rs` import path to emit descriptors alongside meshes.

2. Capture atlas and residency
   - Add `src/gfx/gi/capture/atlas.rs` to manage an RGBA16F atlas (or multiple) storing captured attributes and/or pre‑integrated radiance.
   - Add `src/gfx/gi/capture/recapture.rs` for a scheduler that prioritizes tiles near the camera and those invalidated by TOD changes.
   - Budget knobs: tiles per frame, bytes per frame, distance/importance weights.

3. Runtime captures
   - Add a dedicated capture pipeline that renders material attributes from the tile’s capture view into the atlas (offscreen passes).
   - Capture inputs: current sun/sky parameters (so TOD changes naturally update captures over time).
   - Invalidation: mark tiles when sun direction changes beyond a threshold or sky turbidity/exposure changes.

4. Sampling at ray hits
   - Given a ray hit on a mesh, select the best capture tile (by normal alignment and proximity), project to the tile domain, and fetch attributes/indirect radiance.
   - Start with nearest‑single‑tile sampling; later add multi‑tile blends for continuity.

Phase 3 — Software ray tracing for off‑screen hits
1. Triangle BVH (baseline)
   - Add `src/gfx/rt/bvh/` with a CPU‑built BVH (static) and GPU buffers for traversal if we choose a compute path.
   - Use for diffuse GI and reflection rays when screen‑space march misses.

2. Optional mesh SDFs (detail rays)
   - As a quality toggle, build signed‑distance fields per mesh (coarse resolution). Useful for robust rays on thin details without dense triangle traversal.
   - Module: `src/gfx/rt/sdf/` guarded behind a cargo feature.

3. Hit evaluation modes
   - Fast mode: fetch pre‑captured attributes/indirect from the capture atlas.
   - Quality mode: evaluate BRDF at the actual hit with lights/shadows; use sparingly (e.g., glossy reflections, hero shots).

Phase 4 — Indirect radiance accumulation and stability
1. Diffuse GI gather
   - Per‑pixel or per‑tile rays sample either screen‑space or the BVH path; integrate against capture atlas or hit‑lit shading as configured.
   - Stochastic sampling with blue‑noise sequence; accumulate N samples per frame.

2. Multi‑bounce propagation (trickle updates)
   - Maintain a small “indirect radiance” layer (either in the capture atlas or a lightweight radiance cache texture) and write back partial results each frame.
   - Propagate one extra bounce per frame for slowly converging interiors; clamp with temporal history to ensure stability.

3. Denoisers
   - Separate denoisers for diffuse GI and specular reflections.
   - Temporal‑first (reprojection + history clamp), then a small spatial pass with normal/depth/shading‑rate guides.

Phase 5 — Reflections & translucency polish
- Roughness‑aware reflection ray distribution (GGX importance sampling); adjust ray budgets by roughness.
- Switchable hit mode per material: force “hit‑lighting” for select glossy materials; default to capture‑based lighting elsewhere.
- Translucency reflections as an optional high‑quality switch; document scene constraints.

Phase 6 — Tools, overlays, and debuggability
- Overlays under `src/gfx/debug/`:
  - Capture tile visualization per mesh (counts, coverage, orientation).
  - Atlas inspector: residency heatmap, mips, invalidation reasons.
  - Ray debug: first‑hit buffers, miss reasons (off‑screen, thin geometry, budget capped).
- Perf dials (config struct + hotkeys):
  - Capture tiles per frame; atlas memory cap; distance/importance weights.
  - Toggle detail rays via SDFs; choose fast vs quality hit evaluation.
  - Ray budgets for GI and reflections; denoiser strength.

Time‑of‑day and sky integration
- Single authority for sun/sky parameters: `sky.rs` updates `Globals` and exposes a frame counter + change metrics (e.g., sun angle delta, turbidity delta).
- Capture invalidation: tiles receive a small “age” value and refresh earlier when sun/sky changes exceed thresholds; interiors refresh slower than exteriors.
- SH update cadence: recompute SH‑L2 when TOD changes beyond epsilon; lerp SH across frames to avoid abrupt shifts.

Proposed module map (under `src/gfx/`)
- `gbuffer.rs` — attachments, formats, and MRT wiring.
- `hiz.rs` — depth pyramid build and views.
- `temporal/{reprojection.rs, history.rs}` — shared reprojection + history logic.
- `gi/ssgi.rs` — screen‑space GI pass.
- `gi/capture/{atlas.rs, recapture.rs, sampling.rs}` — capture atlas management and sampling.
- `rt/bvh/{build.rs, traverse.rs}` — triangle BVH path.
- `rt/sdf/{build.rs, trace.rs}` — optional SDF path behind a feature.
- `reflections/{ssr.rs, rt.rs}` — SSR and RT reflections.
- `denoise/{gi.rs, reflections.rs}` — denoisers.
- `debug/{capture_viz.rs, atlas_inspector.rs, rays.rs}` — tools/overlays.

Data formats and buffers (initial suggestions)
- G‑Buffer: RGBA16F base color + emissive, RG16F normal (oct‑encoded), R8 roughness, R8 metalness, R32F depth (read‑only SRV), RG16 motion vectors.
- Capture atlas: RGBA16F for attributes or pre‑shaded radiance; pick one to start (attributes are more flexible; radiance is faster).
- History buffers: GI history (accum), reflection history, clamp neighborhood stats.

Milestones and acceptance checks
- M1 — Foundations
  - G‑Buffer + Hi‑Z build; SSR and SSGI both accumulate temporally without major ghosting.
  - Unit tests for temporal reprojection math and depth pyramid downsample.

- M2 — Indirect capture atlas
  - Import path emits capture descriptors for meshes.
  - Runtime captures update tiles near the camera; atlas viewer overlay shows residency.
  - GI path samples from the atlas on off‑screen hits; TOD changes trickle through via recapture.

- M3 — Software RT path
  - Triangle BVH traversal hooked up for GI/reflection misses; optional SDF toggle compiles.
  - Per‑material switch for fast vs quality hit evaluation.

- M4 — Multi‑bounce + denoisers + tooling
  - Indirect radiance trickle updates deliver visible extra bounce indoors.
  - Denoisers reduce shimmer with stable details; overlays for tiles/atlas/rays help profiling.

Performance and budgets (starting points)
- Capture budget: 32–128 tiles/frame depending on GPU tier; prioritize by distance and motion.
- Ray budgets: GI 1–2 spp/pixel/tile; reflections 0.5–1 spp with temporal reuse.
- Atlas memory: target 64–256 MB for attributes/radiance combined on desktop; lower tiers halve.

Testing strategy
- CPU‑only tests for reprojection matrices, jitter sequences, packing/decoding of normals/motion vectors, and BVH build invariants.
- GPU validation scenes: TOD sweep, glossy sphere grid, interior corridor for multi‑bounce, thin geometry for miss handling.
- Deterministic seeds for stochastic passes; compare accumulations frame‑over‑frame.

Risks and mitigations
- Temporal artifacts: mitigate with reactive masks, history clamping, and neighborhood thresholds.
- Atlas thrash near TOD transitions: budgeted recapture with priority aging and per‑tile cooldowns.
- Overdraw/bandwidth: favor packed G‑Buffer formats and reuse of Hi‑Z for early outs.

Adoption plan
- Keep current forward path running; introduce G‑Buffer as an additional pass that selected pipelines opt into.
- Land SSR first (fast wins), then SSGI; only then introduce the capture atlas.
- Gate advanced features (SDF tracing, quality hit evaluation, multi‑bounce) behind clear runtime config and cargo features.

Notes on provenance and licensing
- This plan draws on public rendering literature and high‑level industry practices. We will not copy engine‑specific code or identifiers; all implementation and naming will remain original to this project.

