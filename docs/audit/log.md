# Phase‑One Refactor Log (PR 1–3)

This log tracks the execution of the first three refactor PRs outlined in `docs/audit/instructions.md`. All changes were no‑behavior refactors (scaffolding, wrappers, and forwarders). CI remained green after each step.

---

## PR 1 — Device/attachments/config scaffolding (render_wgpu)

Changes
- Added `crates/render_wgpu/src/gfx/renderer/device.rs` introducing:
  - `GpuCtx` (device/queue/adapter/limits/features), `SurfaceCtx` (surface/config/size), and `Samplers` (linear + nearest).
  - Provided `from_parts(...)` and `new(...)` helpers (no probing; constructed from existing parts when adopted).
- Added `crates/render_wgpu/src/gfx/renderer/config.rs` centralizing small toggles/constants (`DEFAULT_*`).
- Exported new types from `renderer/mod.rs`:
  - `pub use device::{GpuCtx, SurfaceCtx, Samplers};`
  - `pub use config::*;`
  - Re‑exported attachments as `RenderAttachments` to clarify ownership.

Notes
- The codebase already had `renderer/attachments.rs` and a `renderer/graph` module; no behavior change was made to `Renderer` fields in this pass to minimize churn. Adoption of `GpuCtx/SurfaceCtx/Samplers` will follow in a subsequent PR, per the staged plan.

Validation
- Built with `cargo xtask ci` via pre‑push hook. No clippy or test failures.

---

## PR 2 — Pipelines directory & typed wrappers (mechanical scaffolding)

Changes
- Created `crates/render_wgpu/src/gfx/renderer/pipelines/` with:
  - `common.rs` containing typed wrappers `Bgl<T>` and `Pipeline<T>` plus tag types for passes (Terrain, Instanced, Sky, Ao, Ssgi, Ssr, Bloom).
  - `mod.rs` exporting `common` and a placeholder `Pipelines` grouping for future extraction.

Notes
- Did not replace or touch existing `gfx/pipeline.rs` in this pass to avoid breaking build; this establishes the namespace and types for a follow‑up mechanical move of builders into per‑file modules.

Validation
- CI green.

---

## PR 3 — Framegraph skeleton (forwarder only)

Changes
- Extended `crates/render_wgpu/src/gfx/renderer/graph.rs` with a `FrameGraph::run(...)` method that forwards to the existing `render` implementation (no reordering, encoder/view currently ignored).
- Left the existing `renderer::render::render_impl(...)` intact to preserve call sites; a follow‑up can rename the inner body to `record_frame(...)` and route through the skeleton.

Notes
- The repo already had a minimal frame‑graph for pass I/O validation. This step adds the run‑forwarder stub toward a future pass extraction.

Validation
- CI green; manual smoke not required in automation per repo policy.

---

Summary
- Introduced device/surface/sampler wrappers and a config home, stood up the pipelines namespace with typed wrappers, and added a framegraph run forwarder. All changes are additive and behavior‑neutral, preparing for the next staged moves without disrupting current rendering.
