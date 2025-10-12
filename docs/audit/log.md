# Phase‑One Refactor Log (PR 1–3)

This log tracks the execution of the first three refactor PRs outlined in `docs/audit/instructions.md`. All changes were no‑behavior refactors (scaffolding, wrappers, and forwarders). CI remained green after each step.

---

## PR 7 — platform_winit split (scaffold)

Changes
- Declared new modules in `crates/platform_winit/src/lib.rs`: `app`, `input`, `picker`, `builder_overlay`, `replication`, `telemetry` (empty placeholders).
- Added a small pure mapping function + unit test in `input` to establish a testable seam.
- Left the existing event loop and logic in place for zero behavior change.

Validation
- CI green; input unit test passes.

---

## PR 8 — server_core schedule staged (scaffold)

Changes
- Added nested public modules under `ecs::schedule` for future stages (`stage_input/ai/move/combat/cleanup`).
- Exposed `system_names_for_test()` returning the current schedule’s span labels for verification; added a simple order test.
- No logic moved; run order untouched.

Validation
- CI green; new tests pass.

---

## PR 9 — server_core lib.rs trims (defer heavy moves)

Changes
- No structural moves in this pass to avoid churn; lib.rs already carries concise docs and re‑exports. Trimming/moves will be performed in a later targeted PR.

Validation
- CI green; no behavior change.


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

---

## PR 4 — Split renderer::update by concern (scaffold)

Changes
- Added `gfx/renderer/update/` directory with submodules (`builder.rs`, `projectiles.rs`, `math.rs`, and `destructibles_demo.rs` behind `vox_onepath_demo`).
- Included the original monolithic `update.rs` via `#[path = "../update.rs"] mod legacy;` and re‑exported as `pub(crate)` to keep external call sites unchanged.
- Imported `PcCast` into the renderer namespace to satisfy legacy path references.
- Added `update::math` with two small pure helpers and unit tests (CPU‑only).

Validation
- CI green; no behavior change.

---

## PR 5 — Split gfx::ui into focused modules (scaffold)

Changes
- Added `gfx/ui/` directory with `mod.rs` that includes the original UI via `#[path = "../ui.rs"] mod legacy;` and re‑exports it, plus placeholders `perf.rs`, `help.rs`, `hotbar.rs`.
- Pointed `gfx::ui` to the directory module using `#[path = "ui/mod.rs"]` to avoid ambiguity.
- Added a tiny CPU‑only sanity test under `gfx::ui`.

Validation
- CI green; no behavior change.

---

## PR 6 — tools/model-viewer split scaffolding

Changes
- Declared empty modules `cli/app/viewer/panels/loader/utils` from `main.rs` (no behavior change yet). This sets up the layout for a mechanical move in a follow‑up PR.

Validation
- CI green; tool still builds and runs as before.
