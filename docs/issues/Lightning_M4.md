Title: Lightning M4 — Multi‑bounce Indirect, Denoisers, and Debug Tooling

Goal
- Improve GI quality and stability via multi‑bounce propagation and denoisers tailored to diffuse GI and specular reflections. Ship overlays for capture tiles/atlas and ray debugging to make tuning feasible. Add variance tracking and bounded‑albedo safety to avoid energy blowups.

Scope
- Add a lightweight indirect radiance accumulation layer with trickle updates (one extra bounce per frame). Add temporal‑first denoisers with small spatial filters. Build debug overlays for tiles/atlas/rays. Finalize perf dials.

Planned files and module map (aligned with src/README.md)
- Indirect radiance and propagation
  - `src/gfx/gi/radiance.rs` — Indirect radiance buffers/history, variance buffer, and write‑back from gathers.
  - `src/gfx/gi/propagate.comp.wgsl` — Compute pass to propagate one extra bounce per frame.

- Denoisers
  - `src/gfx/denoise/gi.rs` — Temporal‑first denoiser for diffuse GI; track moments (mean/variance), clamp to neighborhood stats; add 1–2 à‑trous iterations guided by normal/depth.
  - `src/gfx/denoise/reflections.rs` — Temporal‑first denoiser for reflections; roughness‑aware spatial kernel width.
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
- Radiance propagation converges monotonically in a closed‑box test; total energy ≤ analytic bound with diffuse albedo clamped ≤ 0.85; optional Russian roulette on bounce ≥2.
- Denoisers reduce >60% high‑frequency variance after 16 frames (measured by variance drop or PSD proxy) without biasing edges (∆E < 2 on edge tests).
- Overlays render: tile placement, atlas residency heatmaps, and ray first‑hit/miss reasons with categories: OFFSCREEN, THIN_GEO, BUDGET, BACKFACE, ALPHA_REJECT.
- Perf dials exposed to tweak budgets, denoiser strength, and propagation cadence; counters visible: rays launched, % history kept, tiles recaptured, BVH steps avg/max.

Detailed tasks
- Radiance accumulation and propagation
  - Implement `gi/radiance.rs` buffers and history; define RGBA16F for radiance and R16F for variance.
  - Implement `propagate.comp.wgsl` to push one additional bounce per frame; clamp via temporal history; bound diffuse albedo ≤ 0.85; optionally apply Russian roulette on bounce ≥2.

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
- Radiance buffers: `RGBA16F` with exposure‑consistent accumulation; variance: `R16F`.

Tests
- Temporal denoiser unit tests: clamp behavior on synthetic step edges; variance reduction sanity.
- Radiance propagation: energy doesn’t explode for a closed box test; monotonic convergence under bounded albedo; Russian roulette doesn’t bias mean beyond tolerance.

Out of scope
- Hardware RT integration; path‑traced modes; translucency refraction.

Housekeeping
- Keep module docblocks and rustdoc complete; ensure `clippy` clean.
