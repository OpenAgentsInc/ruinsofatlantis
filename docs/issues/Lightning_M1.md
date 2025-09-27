Title: Lightning M1 — Foundations: G‑Buffer, Z‑MAX Hi‑Z, Temporal, SSR/SSGI

Goal
- Establish the core rendering infrastructure for stable environment lighting: G‑Buffer pass, depth pyramid, temporal reprojection/history, and baseline screen‑space reflections (SSR) and screen‑space diffuse GI (SSGI). Wire all of this to our existing sun/sky/time‑of‑day system.

Scope
- Add new render passes and shared utilities without breaking current forward paths. Keep modules cohesive and documented per repo guidelines.

Planned files and module map (aligned with src/README.md)
- `src/gfx/gbuffer.rs` — Create and manage MRT targets; record G‑Buffer pass.
- `src/gfx/gbuffer.wgsl` — VS/FS writing: normal (ws), albedo, packed roughness/metalness, emissive (optional), motion vectors.
- `src/gfx/hiz.rs` — Build hierarchical Z‑MAX depth pyramid via compute over linearized depth; provide SRV views per mip.
- `src/gfx/hiz.comp.wgsl` — Compute shader building Z‑MAX mip chain from an R32F copy of linear view‑space depth (not the depth attachment directly).
- `src/gfx/temporal/mod.rs` — Module index and shared types.
- `src/gfx/temporal/reprojection.rs` — Jitter, prev matrices, velocity usage, history clamp masks.
- `src/gfx/temporal/history.rs` — History textures management + neighborhood clamp.
- `src/gfx/temporal/resolve.rs` — Optional: shared temporal resolve helpers (for future TAA).
- `src/gfx/reflections/ssr.rs` — Screen‑space reflections pass: ray march in Hi‑Z, refine, temporal accumulate.
- `src/gfx/ssr.wgsl` — WGSL for SSR ray‑march + resolve.
- `src/gfx/gi/ssgi.rs` — Screen‑space diffuse GI: cosine‑weighted rays, temporal accumulation, fallback to sky SH.
- `src/gfx/ssgi.wgsl` — WGSL for SSGI.
- `src/gfx/pipeline.rs` — Extend to create pipelines and bind group layouts for the above passes.
- `src/gfx/mod.rs` — Integrate passes into init/resize/render; add a `LightingConfig` to control toggles.
- `src/gfx/shaders/common.wgsl` — Shared bindings (globals/history/Hi‑Z), matrices, jitter, frame index, blue‑noise seed.
- Tests: co‑located `#[cfg(test)]` in `temporal/reprojection.rs` and `hiz.rs`.

Connections to existing hierarchy (read src/README.md)
- `src/gfx/sky.rs` already drives sun dir and SH. Ensure new passes consume `Globals` consistently.
- Avoid bloating `shader.wgsl`; add dedicated WGSL files per pass and load them in `pipeline.rs`.
- Update `src/README.md` sections under `gfx/` to document each new module’s responsibility.

Acceptance criteria
- G‑Buffer renders for terrain + wizards without visual regressions in the current scene.
- Z‑MAX Hi‑Z built each frame from linear R32F depth; SSR/SSGI consume it for marching.
- Temporal reprojection stability: >90% pixels keep history during a slow pan; emissive streaking ≤1 px using a reactive mask (validated by a debug counter).
- SSR: mirror floor shows continuous reflection across camera cuts within 8 frames; no laddering on grazing angles; miss path falls back to SSGI‑reflected color (if enabled) then sky.
- SSGI: indoor wall shows subtle colored bounce from floor; speckle visually negligible after 16 frames.
- Unit tests: reprojection math, oct encode/decode error bound, motion vector pixel tolerance under jitter, and Z‑MAX pyramid invariants.

Detailed tasks
- G‑Buffer
  - Implement `src/gfx/gbuffer.rs` with MRT creation (formats suggested below).
  - Author `src/gfx/gbuffer.wgsl` VS/FS; output motion vectors using current vs previous clip positions.
  - Modify terrain/wizard material paths to supply base PBR params for the G‑Buffer.
  - Pipeline wiring: extend `src/gfx/pipeline.rs` to compile/link the G‑Buffer pass.

- Hi‑Z
  - Copy/resolve the `Depth32Float` attachment into an `R32Float` linearized view‑space depth texture.
  - Implement `src/gfx/hiz.rs` and `src/gfx/hiz.comp.wgsl` to build a Z‑MAX mip chain on that R32F texture.
  - Store reciprocal texel size per mip; expose `choose_mip(step_len_pixels)` to select a marching mip.

- Temporal reprojection
  - Add `src/gfx/temporal/reprojection.rs` and `history.rs` with docblocks and public helpers.
  - Maintain previous view‑proj, `prev_jitter` and `curr_jitter`; remove jitter before computing prev UVs.
  - Produce disocclusion masks from depth deltas and velocity magnitude; implement reactive masks for emissive/thin geometry.
  - Provide a shared bind layout for history textures; update in `pipeline.rs`.

- SSR
  - Implement `src/gfx/reflections/ssr.rs` and `src/gfx/ssr.wgsl`.
  - Roughness‑aware step counts; parallax‑correct the view‑space ray; reject backfaces if needed.
  - Blue‑noise ray jitter; binary search refine; temporal accumulate; clamp against 3× neighborhood stddev in luma space.
  - Miss path: SSR → optional SSGI reflected color → sky.

- SSGI
  - Implement `src/gfx/gi/ssgi.rs` and `src/gfx/ssgi.wgsl`.
  - Cosine‑weighted rays in view space; bias first step along normal by epsilon to avoid self‑hits.
  - Use Hi‑Z to adapt step size; 1–2 spp per frame; temporal accumulation; fall back to sky SH weighted by an AO‑like term.

- Integration and config
  - Add a `LightingConfig` to `src/gfx/mod.rs` with knobs:
    - `jitter_sequence` (Halton(2,3) with Cranley–Patterson rotation)
    - `temporal_alpha_{ssr,ssgi}`, `clamp_k`, `reactive_boost`
    - `ssr_thickness`, `ssr_roughness_cutoff`, `ssr_max_steps`
    - `ssgi_num_rays`, `ssgi_step_bias`, `ssgi_max_steps`
  - Ensure all public types/functions have rustdoc; add brief module‑level docblocks.
  - Update `src/README.md` to reflect new files and responsibilities.
  - sRGB handling: all G‑Buffer targets are linear; only the swapchain is sRGB. Document in `pipeline.rs` and `common.wgsl`.

Suggested formats (tune per GPU tier)
- Albedo: `RGBA8Unorm` (linear)
- Normal (oct‑encoded): `RG16Snorm`
- Roughness/Metalness (packed): `RG8Unorm`
- Emissive (optional HDR): `RGBA16F` or pack LDR emissive in albedo alpha
- Motion vectors: `RG16F`
- Depth: `Depth32Float` + linear copy `R32Float` for Hi‑Z

Tests
- `temporal/reprojection.rs`: verify reprojection of a static point is identity with zero jitter; verify sub‑pixel jitter sequences map to expected offsets; disocclusion mask flips when depth deltas exceed threshold.
- `hiz.rs`: CPU reference for Z‑MAX downsample — mip0 equals linear depth; mip1 equals max over 2×2 blocks for a 4×4 synthetic plane at two depths.
- `gbuffer.wgsl` helpers: oct encode/decode round‑trip L2 error < 1e‑3.
- Motion vectors: animated transform (translate+rotate) yields expected pixel offsets under known jitter.

Out of scope
- Off‑screen ray hits (defer to Lightning M2/M3).
- Multi‑bounce GI and denoisers (Lightning M4).

Housekeeping
- Follow dependency policy: if any new crate is needed, add via `cargo add`, not manual edits.
- Keep the repo compiling; run `cargo fmt` and `cargo clippy -- -D warnings`.

Cross‑cutting engineering (applies to all Lightning milestones)
- Frame graph: model passes (G‑Buffer, Hi‑Z, SSR, SSGI, Capture, RT, Denoisers) as nodes with explicit read/write resources to manage lifetimes and aliasing.
- Bind group conventions (document in `common.wgsl` and `pipeline.rs`):
  - 0 = Globals (camera, jitter, exposure, TOD, frame index)
  - 1 = Per‑view history & Hi‑Z
  - 2 = Material/textures
  - 3 = Pass‑local (SSR/SSGI histories, etc.)
- Resolution scaling hooks: allow half‑res SSR/SSGI with Catmull–Rom upsample + neighborhood clamping; add `independent_resolution: { ssr, ssgi }` to `LightingConfig`.
- Color management: pick a tone‑mapper (ACES/Hable) and consider pre‑exposure for stable temporal blending; document early.
- Sample scenes & goldens under `data/tests/`: mirror floor + emissive sign; colored floor + white wall; thin bars in front of far wall. Render 8–16 frames and assert SSIM/variance thresholds in CI.
