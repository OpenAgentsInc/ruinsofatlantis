Title: Lightning M4 — Multi‑bounce Indirect, Denoisers, and Debug Tooling

Goal
- Improve GI quality and stability via multi‑bounce propagation and denoisers tailored to diffuse GI and specular reflections. Ship overlays for capture tiles/atlas and ray debugging to make tuning feasible.

Scope
- Add a lightweight indirect radiance accumulation layer with trickle updates (one extra bounce per frame). Add temporal‑first denoisers with small spatial filters. Build debug overlays for tiles/atlas/rays. Finalize perf dials.

Planned files and module map (aligned with src/README.md)
- Indirect radiance and propagation
  - `src/gfx/gi/radiance.rs` — Indirect radiance buffers/history and write‑back from gathers.
  - `src/gfx/gi/propagate.comp.wgsl` — Compute pass to propagate one extra bounce per frame.

- Denoisers
  - `src/gfx/denoise/gi.rs` — Temporal‑first denoiser for diffuse GI; small spatial pass with normal/depth guides.
  - `src/gfx/denoise/reflections.rs` — Temporal‑first denoiser for reflections; roughness‑aware spatial kernel.
  - `src/gfx/denoise/gi_denoise.comp.wgsl` and `src/gfx/denoise/reflections_denoise.comp.wgsl` — Compute shaders.

- Debug tooling
  - `src/gfx/debug/rays.rs` — First‑hit buffers, miss reason visualization (off‑screen, thin geo, budget cap).
  - Reuse/extend: `src/gfx/debug/capture_viz.rs`, `src/gfx/debug/atlas_inspector.rs` (from Lightning M2).

- Integration
  - `src/gfx/pipeline.rs` — Pipelines/bind groups for denoisers and propagation.
  - `src/gfx/mod.rs` — Frame graph order: gather -> write‑back -> propagate -> denoise -> composite.

Connections to existing hierarchy (read src/README.md)
- Works with G‑Buffer, Hi‑Z, temporal history from M1; capture atlas and RT paths from M2/M3.
- Update `src/README.md` sections under `gfx/` to describe radiance cache, denoisers, and debug overlays.

Acceptance criteria
- Indirect lighting shows visible extra bounce indoors as frames accumulate, without large flicker.
- Denoisers reduce shimmer in GI and reflections; temporal stability maintained during slow camera motion.
- Overlays render: tile placement, atlas residency heatmaps, and ray first‑hit/miss reasons.
- Perf dials exposed to tweak budgets, denoiser strength, and propagation cadence.

Detailed tasks
- Radiance accumulation and propagation
  - Implement `gi/radiance.rs` buffers and history; define a compact format (e.g., RGBA16F) for accumulations.
  - Implement `propagate.comp.wgsl` to push one additional bounce per frame; clamp via temporal history.

- Denoisers
  - Implement temporal reprojection hooks and clamping reuse; add small spatial kernels guided by normal/depth/shading rate.
  - Expose parameters in `LightingConfig` (temporal alpha, clamp strength, spatial radius).

- Debug overlays
  - Implement `debug/rays.rs` showing first‑hit normal/depth and miss reason categories.
  - Extend `capture_viz.rs` and `atlas_inspector.rs` with per‑tile age/invalidations and scheduler scores.

- Integration and config
  - Add perf dials to `LightingConfig`: radiance update budget, propagate cadence (frames/bounce), denoiser strengths.
  - Update `src/README.md` to reflect modules and usage.

Suggested formats
- Radiance buffers: `RGBA16F` with exposure‑consistent accumulation; separate variance buffer optional.

Tests
- Temporal denoiser unit tests: clamp behavior on synthetic step edges; variance reduction sanity.
- Radiance propagation: energy doesn’t explode for a closed box test; monotonic convergence under bounded albedo.

Out of scope
- Hardware RT integration; path‑traced modes; translucency refraction.

Housekeeping
- Keep module docblocks and rustdoc complete; ensure `clippy` clean.

