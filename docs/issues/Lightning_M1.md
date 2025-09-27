Title: Lightning M1 — Foundations: G‑Buffer, Hi‑Z, Temporal, SSR/SSGI

Goal
- Establish the core rendering infrastructure for stable environment lighting: G‑Buffer pass, depth pyramid, temporal reprojection/history, and baseline screen‑space reflections (SSR) and screen‑space diffuse GI (SSGI). Wire all of this to our existing sun/sky/time‑of‑day system.

Scope
- Add new render passes and shared utilities without breaking current forward paths. Keep modules cohesive and documented per repo guidelines.

Planned files and module map (aligned with src/README.md)
- `src/gfx/gbuffer.rs` — Create and manage MRT targets; record G‑Buffer pass.
- `src/gfx/gbuffer.wgsl` — VS/FS writing: normal (ws), albedo, roughness, metalness, emissive, motion vectors.
- `src/gfx/hiz.rs` — Build hierarchical depth pyramid via compute; provide SRV views per mip.
- `src/gfx/hiz.comp.wgsl` — Compute shader for min‑reduce/downsample from full‑res depth.
- `src/gfx/temporal/mod.rs` — Module index and shared types.
- `src/gfx/temporal/reprojection.rs` — Jitter, prev matrices, velocity usage, history clamp masks.
- `src/gfx/temporal/history.rs` — History textures management + neighborhood clamp.
- `src/gfx/reflections/ssr.rs` — Screen‑space reflections pass: ray march in Hi‑Z, refine, temporal accumulate.
- `src/gfx/ssr.wgsl` — WGSL for SSR ray‑march + resolve.
- `src/gfx/gi/ssgi.rs` — Screen‑space diffuse GI: cosine‑weighted rays, temporal accumulation, fallback to sky SH.
- `src/gfx/ssgi.wgsl` — WGSL for SSGI.
- `src/gfx/pipeline.rs` — Extend to create pipelines and bind group layouts for the above passes.
- `src/gfx/mod.rs` — Integrate passes into init/resize/render; add a `LightingConfig` to control toggles.
- Tests: co‑located `#[cfg(test)]` in `temporal/reprojection.rs` and `hiz.rs`.

Connections to existing hierarchy (read src/README.md)
- `src/gfx/sky.rs` already drives sun dir and SH. Ensure new passes consume `Globals` consistently.
- Avoid bloating `shader.wgsl`; add dedicated WGSL files per pass and load them in `pipeline.rs`.
- Update `src/README.md` sections under `gfx/` to document each new module’s responsibility.

Acceptance criteria
- G‑Buffer renders for terrain + wizards without visual regressions in the current scene.
- Hi‑Z built each frame; SSR and SSGI use it for marching.
- Temporal reprojection stable on camera motion: limited ghosting thanks to history clamp; emissive flicker handled by reactive mask.
- SSR: visible reflections for on‑screen hits; falls back to sky when miss.
- SSGI: subtle diffuse bounce; falls back cleanly to sky SH.
- Unit tests: reprojection math (matrix/game units), and depth pyramid downsample invariants.

Detailed tasks
- G‑Buffer
  - Implement `src/gfx/gbuffer.rs` with MRT creation (formats suggested below).
  - Author `src/gfx/gbuffer.wgsl` VS/FS; output motion vectors using current vs previous clip positions.
  - Modify terrain/wizard material paths to supply base PBR params for the G‑Buffer.
  - Pipeline wiring: extend `src/gfx/pipeline.rs` to compile/link the G‑Buffer pass.

- Hi‑Z
  - Implement `src/gfx/hiz.rs` and `src/gfx/hiz.comp.wgsl` for mip downsample (min reduction for depth).
  - Build after G‑Buffer; expose a helper to query appropriate mip for step size.

- Temporal reprojection
  - Add `src/gfx/temporal/reprojection.rs` and `history.rs` with docblocks and public helpers.
  - Maintain previous view‑proj and jitter; produce clamp masks using neighborhood statistics.
  - Provide a shared bind layout for history textures; update in `pipeline.rs`.

- SSR
  - Implement `src/gfx/reflections/ssr.rs` and `src/gfx/ssr.wgsl`.
  - Use roughness‑aware step counts; binary search refine; temporal accumulate with history.
  - Fallback to environment (sky) when miss.

- SSGI
  - Implement `src/gfx/gi/ssgi.rs` and `src/gfx/ssgi.wgsl`.
  - Cosine‑weighted rays in screen space; temporal accumulation; fallback to sky SH when miss.

- Integration and config
  - Add a `LightingConfig` to `src/gfx/mod.rs` with toggles: enable_ssr, enable_ssgi, temporal_strength, max_ssr_steps, etc.
  - Ensure all public types/functions have rustdoc; add brief module‑level docblocks.
  - Update `src/README.md` to reflect new files and responsibilities.

Suggested formats (tune per GPU tier)
- Albedo+Emissive: `RGBA16F`
- Normal (oct‑encoded): `RG16F`
- Roughness: `R8`
- Metalness: `R8`
- Motion vectors: `RG16`
- Depth: `D32Float` (SRV read for Hi‑Z build)

Tests
- `temporal/reprojection.rs`: verify reprojection of a static point is identity with zero jitter; verify sub‑pixel jitter sequences map to expected offsets.
- `hiz.rs`: CPU reference for a small depth image to verify first two mip levels (min) match compute shader.

Out of scope
- Off‑screen ray hits (defer to Lightning M2/M3).
- Multi‑bounce GI and denoisers (Lightning M4).

Housekeeping
- Follow dependency policy: if any new crate is needed, add via `cargo add`, not manual edits.
- Keep the repo compiling; run `cargo fmt` and `cargo clippy -- -D warnings`.

