# Refactor Log — Phase One (PR 1–11)

This log tracks the execution of phase‑one refactor PRs outlined in `docs/audit/instructions.md` (PR 1–11). All changes were no‑behavior refactors (scaffolding, wrappers, forwarders). CI remained green after each step.

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

---

## PR 10 — net_core snapshot split (scaffold)

Changes
- Converted `crates/net_core/src/snapshot.rs` into a directory module:
  - Added `crates/net_core/src/snapshot/mod.rs` with domain stubs (`encode.rs`, `actors.rs`, `projectiles.rs`, `destructibles.rs`, `hud.rs`) and a legacy include.
  - Kept original implementation via a `legacy` module re-exporting the body to preserve the API with zero behavior change.
- Left existing tests intact (round‑trips and delta checks pass).

Validation
- CI green; no wire format changes.

---

## PR 11 — Move vox demo under gated path

Changes
- Added `crates/render_wgpu/src/gfx/demo/vox_onepath.rs` that includes the original demo implementation.
- Changed `gfx::mod` to export `gfx::demo::vox_onepath` under `vox_onepath_demo` feature, moving the demo out of the core path.
- Default build remains unchanged; the demo compiles only when the feature is enabled.

Validation
- CI green; feature off by default per existing config.

---

## Phase‑Two — PR 12: Framegraph core (builder, resources, validation)

Changes
- Introduced a real framegraph builder in `gfx/renderer/graph.rs`:
  - Added `ImageKind`, `Handle<Img>`, `GraphBuilder`, `PassDecl`, `ExecCtx`, and `Graph` types.
  - Implemented per‑pass hazard validation (read+write of same image panics in debug) and a simple `compile()` that preserves declaration order.
  - Added a `run_forwarder` that builds a single Monolith pass and forwards to the legacy render path to keep behavior parity.
- Kept the existing static graph (`graph_for`) and its tests intact for compatibility.
- Added CPU‑only tests for hazard detection and declaration‑order stability.

Validation
- CI green; visuals unchanged (Monolith calls the same render path).

---

## Phase‑Two — PR 13: Extract Present and Sky (declarations)

Changes
- Added `gfx/renderer/passes_graph.rs` with scaffolded pass declarations for `SkyPass` and `PresentPass` using the new `GraphBuilder` API.
- For now, these passes register no‑op exec closures (no behavior change). They declare write intent to the color target to establish IO.
- Kept present lifecycle as‑is in the legacy path (`frame.present()`), per low‑churn guidance.

Validation
- CI green; visuals unchanged.

---

## Phase‑Two — PR 14: Extract Main (declaration)

Changes
- Extended `passes_graph.rs` with `MainPass::declare(...)` that writes to color and depth handles, matching the intended IO of the main scene pass.
- Did not move draw code yet; execution remains in the legacy path. This sets the shape for later wiring while keeping diffs small.

Validation
- CI green; visuals unchanged.

---

## Phase‑Two — PR 15: Particles + UI (execute via graph)

Changes
- Added pass exec closures in `gfx/renderer/passes_graph.rs`:
  - `ParticlesPass`: opens a render pass targeting offscreen scene color and calls `renderer.draw_particles(...)` when `fx_count > 0`.
  - `UiPass`: calls `hud.queue(device, queue)` then draws the HUD into offscreen scene color.
- Extended `renderer::graph::ExecCtx` to include `swap_view` and minimal accessors `device()`, `queue()`, `surface_config()`.
- Implemented `Graph::execute(renderer, encoder, swap_view)` to iterate and run pass closures.
- Replaced the temporary `FrameGraph::run_particles_ui` helper (removed) with a real graph build/execute in `renderer::render`.

Validation
- CI green locally; visuals unchanged (particles and HUD still render correctly).

---

## Phase‑Two — PR 16: Offscreen hdr_color + real Present pass

Changes
- Switched the scene to render to offscreen (`attachments.scene_view`) unconditionally in `renderer::render` (Sky/Main/overlays paths now target offscreen).
- Added a real `PresentPass` exec closure in `passes_graph.rs` that composites offscreen color to the swapchain using the existing `present_pipeline`/`present_bg`.
- `renderer::render` now builds a per‑frame graph with `Particles → UI → Present` and calls `Graph::execute`, then submits and presents.

Validation
- CI green locally; behavior parity maintained. Present path now explicitly composites offscreen → swapchain.

---

## Phase‑Two — PR 17: Draw‑list builder (CPU‑only) + tests

Changes
- Added `gfx/renderer/draw_list.rs` with a pure `DrawList` builder that groups contiguous `DrawItem`s by `DrawKey` into `DrawBatch`es. This is deterministic and has no `wgpu` dependencies.
- Exported from `renderer::mod` so future passes (Main) can adopt it with low churn.
- Wrote unit tests that verify:
  - Contiguous identical keys are merged with summed counts.
  - Different keys do not merge across boundaries.
  - Stable ordering of batches.

Validation
- All tests pass under `cargo test`. No behavior change in rendering yet; integration into Main will follow in a later PR.

---

## Phase‑Two — PR 18: BindGroup cache scaffold

Changes
- Added `gfx/renderer/bindgroups.rs` with a simple `BgCache` and `BgKey`:
  - `BgKey { layout_hash, ids }` captures layout identity and resource ids.
  - `BgCache::get_or_create(key, make)` returns a cached BG or inserts via `make`, tracking `hits/misses/evictions` with FIFO/LRU-ish eviction at a fixed capacity.
- Exported from `renderer::mod` so passes and materials can adopt it later.

Validation
- Compiles cleanly with clippy `-D warnings`. Not yet adopted by passes; no behavior change.

---

## Phase‑Two — PR 19: Upload ring scaffold

Changes
- Added `gfx/renderer/upload.rs` with a simple per‑frame bump allocator:
  - `UploadRing::new(device, frames, initial_size, usage, label)` creates one buffer per frame.
  - `next_frame()` rotates the active buffer and resets the cursor.
  - `allocate(queue, data, align)` writes bytes to the current frame buffer at an aligned offset; grows the buffer if needed (resets cursor on growth).
  - Returns `UploadSlice { buffer, offset, size }` for downstream binds.
- Exported from `renderer::mod` for later adoption in material/uniform updates.

Validation
- Clippy `-D warnings` clean; unit test added for alignment helper. No runtime adoption yet; behavior unchanged.

---

## Phase‑Two — PR 20: Centralized resize/rebuild bus

Changes
- Added `gfx/renderer/rebuild_bus.rs` providing a simple `RebuildBus` to notify subsystems on resize/attachment changes.
- Renderer now owns `rebuild_bus` and registers listeners during init to rebuild all sized bind groups (Present, Post AO, SSGI, SSR) from current `attachments` and samplers.
- Updated `resize_impl` to rebuild attachments/Hi‑Z/G‑Buffer, then dispatch the bus once instead of hand‑recreating each bind group inline.

Validation
- Behavior remains the same; only the call site changes. clippy/tests are green.

---

## Phase‑Two — PR 21: UI pass finalization

Changes
- Ensured the `UI` pass exec closure is branch‑free (just queue + draw). The toggles/text building logic remains in the `ui` module and render prep.
- Added a CPU‑only test that the HUD vertex count model is deterministic for a small configuration (3 slots) and scales with slot count.

Validation
- Tests pass; UI visuals unchanged; pass remains a thin draw wrapper.

---

## Phase‑Two — PR 22: Shims/doc polish

Changes
- Improved rustdoc for `renderer::graph` and `passes_graph` to document scope and IO.
- Confirmed remaining `#[path]` shims were removed in earlier PRs; kept necessary compatibility imports only.

Validation
- Clippy/doc build clean; no behavior change.

---

## Phase‑Two — PR 16: Post suite + offscreen image (prep)

Changes
- Added cross‑pass monotonic hazard validation (no write after any read) in the framegraph compiler to catch ordering mistakes early.
- Present lifecycle remains unchanged for now; introduction of `hdr_color` and non‑swapchain compositing will come with Post extraction.

Validation
- CI green; visuals unchanged.
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
## Phase‑Two — PR 23: Main uses DrawList (scaffold)

Changes
- Added `renderer::passes::main.rs` scaffold to host Main’s execution and prepare for DrawList‑driven batching.
- Kept legacy Main draws intact; no behavior change yet. DrawList module and tests already exist.

Validation
- Visual parity. clippy/tests green.

---

## Phase‑Two — PR 29: Main under pass (prep)

Changes
- Declared `MainPass` in the per‑frame graph and added a stats placeholder so perf UI shows a stable row.
- Legacy Main draws remain in `render.rs` for now to keep behavior parity; extraction will bind Main into the pass next.

Validation
- Visual parity; graph order now includes Main (stub) before Particles.

---

## Phase‑Two — PR 24: BgCache in more hotspots

Changes
- Adopted `BgCache` for zombie palettes bind groups in the render path to avoid rebuilding when buffer identity is unchanged across frames.
- Adopted `BgCache` for resize‑recreated BGs via `rebuild_bus` (Present, Post AO, SSGI depth/scene, SSR depth/scene) keyed by view/sampler ids.

Validation
- Visual parity; cache keys stable; clippy/tests green.

---

## Phase‑Two — PR 25: UploadRing in more hotspots

Changes
- Integrated `UploadRing` for DK instance updates (staged copy) and kept uniform writes direct where encoder borrowing would conflict. Considered palette/instance updates; deferred until they move under passes to avoid borrow/encoder lifetime issues.
- `uploads.next_frame()` runs at frame start before recording commands.

Validation
- Visual parity; no mixed update path for the same buffer in a frame; clippy/tests green.

---

## Phase‑Two — PR 26: Present owns acquire/present

Changes
- Moved swapchain acquire/present into the Present pass exec closure. The render path no longer acquires or presents directly.
- `Graph::execute` API simplified (no swap view argument). Legacy debug paths use the offscreen view for drawing when needed.

Validation
- Visual parity. Hazard validation intact (Present reads `hdr_color`). clippy/tests green.

---

## Phase‑Two — PR 31: IO correctness & Present error handling

Changes
- Passes now consistently access views via `ExecCtx::view_color/view_depth` (Particles/UI already adopted; Main executes through `pass_main` which targets `attachments.scene_view/depth_view` identically).
- Corrected Particles pass IO to only `.writes(color)` (no depth claims).
- Strengthened `Present` error handling in `passes_graph.rs`:
  - Handles `SurfaceError::Lost|Outdated` by invoking `renderer::resize::resize_impl` with current size.
  - Treats `Timeout` as a soft skip for the frame; `OutOfMemory` logs an error and returns.

Validation
- Visual parity; resize/minimize/maximize recovers without panic locally.
- clippy/tests green.

---

## Phase‑Two — PR 32: BgCache correctness + tests

Changes
- Fixed `BgCache::get_or_create` to return by key and refresh recency; proper FIFO/LRU-ish eviction.
- Added unit tests for counts and eviction. Tests are CI-friendly and skip gracefully if no adapter is available in the environment.

Validation
- `cargo test -p render_wgpu` passes; cache tests validated hits/misses/evictions.
- Full `xtask ci` green.

---

## Phase‑Two — PR 33: Main batching plumbing (behavior-neutral)

Changes
- Updated Main pass `RenderStats` to report `batches = draws` conservatively to reflect one-batch-per-draw until DrawList integration lands.
- Left draw code intact (no visual changes). DrawList module and tests already exist and will be adopted next.

Validation
- Visual parity.
- clippy/tests green.

---

## Phase‑Two — PR 36: Split pipelines (mechanical)

Changes
- Added per-pass pipeline modules under `gfx/renderer/pipelines/*` with behavior-neutral re-exports to `gfx/pipeline.rs`:
  - `{present,sky,terrain,instanced,post_ao,ssgi,ssr,bloom}.rs` plus `common.rs` and `mod.rs`.
- Introduced a `Pipelines` grouping (empty scaffolding for now) and exported the namespace from `renderer::mod`.
- Left all call-sites intact (continue to use `gfx::pipeline::*`); this sets up a no-churn path to migrate builders later.

Validation
- `xtask ci` green; no public API changes at call-sites.

---

## Phase‑Two — PR 37: ExecCtx::pipelines() + typed handle scaffolding

Changes
- Added `Renderer::pipelines: renderer::pipelines::Pipelines` field and initialized it in `init.rs` (default).
- Added `ExecCtx::pipelines()` accessor returning `&Pipelines` (not yet used by passes).
- Exported `renderer::pipelines` from `renderer/mod.rs`.

Validation
- `xtask ci` green; no behavior changes; accessor unused for now.

---

## Phase‑Two — PR 39: UploadRing adoption (more hotspots)

Changes
- Switched per-frame uploads for `globals_buf`, `sky_buf`, `lights_buf`, and the normal-path `shard_model_buf` to use the staging `UploadRing` + `copy_buffer_to_buffer`.
- Kept the debug-only shard UBO update inside the open pass via `queue.write_buffer` to avoid encoder borrows; no mixed write path for the same buffer in one frame.
- Added unit test `next_frame_resets_cursor()` under `renderer/upload.rs` (device-backed; skips if no adapter) and exposed test-only getters.

Validation
- `xtask ci` green; no visual changes. Grep confirms no buffer uses both write paths in the same frame.

---

## Phase‑Two — PR 38: Main adopts DrawList batching (behavior‑neutral)

Changes
- Added conservative batch counting in `pass_main` without changing draw behavior. We compute a key per draw as `(pipeline_id, material_id, mesh_id)` and count a new batch when the key changes; order matches legacy.
- Updated `RenderStats` for Main to report real `batches` via a renderer field (`main_batch_count_last`). Draws are still issued exactly as before.

Validation
- `xtask ci` green locally; visuals unchanged. Perf HUD shows `batches ≤ draws` for Main.

---

## Phase‑Two — PR 40: RebuildBusCore<T> + tests

Changes
- Extracted a generic `RebuildBusCore<T>` with `new/register/run_all` and defined `type RebuildBus = RebuildBusCore<Renderer>`.
- Added CPU-only unit test `listeners_run_in_order` using `RebuildBusCore<u32>` to verify deterministic order.

Validation
- `xtask ci` green.

---

## Phase‑Two — PR 41: File splits (mechanical)

Changes
- Model viewer: moved UI text helpers (glyph5x7_rows, UiVertex, build_text_quads) from `main.rs` into `panels.rs` and imported them, keeping `main.rs` as the bootstrap + orchestration. The remaining split modules (`cli/app/viewer/loader/utils`) are present and compile; further moves can proceed incrementally with zero churn.
- platform_winit: module façade (`pub mod {app,input,picker,builder_overlay,replication,telemetry}`) is in place and compiles. Large bodies remain in their legacy file; planned next step is a mechanical `lib.rs` → `app.rs` move with `pub use app::*` to preserve the public API.

Validation
- `xtask ci` green; no behavior change. Tools and platform crates still build/run.

## Phase‑Two — PR 35: Extract Post Suite (behavior‑neutral)

Changes
- Added post pass helpers in `gfx/renderer/passes.rs`:
  - `pass_ao`, `pass_ssgi`, `pass_ssr`, and new `pass_bloom` target `attachments.scene_view` and use existing BGs/pipelines; `pass_blit_scene_read` remains to prep `SceneRead` when needed.
- Declared post passes in `gfx/renderer/passes_graph.rs` with per‑pass `RenderStats`:
  - `PostAoPass` (reads depth, writes color), `BlitSceneReadPass` (no graph IO; copies color→read), `SsgiPass` (reads depth, writes color), `SsrPass` (reads depth, writes color), `BloomPass` (writes color).
- Wired graph order in `gfx/renderer/render.rs`:
  - `Main → Particles → UI → PostAO → BlitSceneRead → SSGI → SSR → Bloom → Present`.
- Legacy monolith code left intact; graph path now executes post passes. Visuals unchanged.

Validation
- `xtask ci` green; all unit tests and WGSL validation pass.
- Manual smoke: post overlays render as before; perf HUD shows Post rows with ms/draws.

---

## Phase‑Three — PR 42: Graph Views API (behavior‑neutral)

Changes
- Extended `renderer::graph::Graph` to retain declared `images` and a per‑frame `views` array.
- Updated `ExecCtx` to expose `view_color(handle)` and `view_depth(handle)` backed by the graph’s `views` slice.
- In `Graph::execute`, populated `views` by aliasing handles to current attachments (`scene_view` / `depth_view`) to keep behavior identical.
- Passes now consistently consume graph views (Particles/UI already adopted; others remain unchanged behaviorally).

Validation
- `cargo clippy -D warnings` and `cargo test` green.
- Visual parity verified locally; no change in Present or Main paths.

---

## Phase‑Three — PR 43: Allocation Plan & Liveness (no aliasing yet by default)

Changes
- Computed simple liveness intervals (`first/last` touching pass) and per‑image `TextureUsages` during `Graph::execute`.
- Added an optional per‑image instantiation path gated by `RA_GRAPH_ALLOC=1` that creates `wgpu::Texture`s 1:1 from `ImageKind` and usage flags, filling `views` with those texture views.
- Default remains attachment aliasing for parity (env var off).

Validation
- Logic‑only; default path unchanged. Unit tests unaffected; CI green.
- Manual smoke with `RA_GRAPH_ALLOC=1` keeps passes executing; Present still composites attachments as expected.

---

## Phase‑Three — PR 44: Optional Aliasing (interval packing, env‑gated)

Changes
- Implemented a simple interval packing allocator behind `RA_GRAPH_ALIASING=1` (effective only when `RA_GRAPH_ALLOC=1` is also set):
  - Reuses textures for images with identical descriptors (format/size/samples/usages) and non‑overlapping lifetimes.
- Falls back to 1:1 instantiation when aliasing is disabled; logs can be enabled via `RA_GRAPH_TRACE=1` (future enhancement).

Validation
- Feature is off by default; CI green on default path.
- Local smoke with both env vars enabled shows no validation errors.

---

## Phase‑Three — PR 45: MSAA threading (scaffold)

Changes
- Threaded `attachments.sample_count` through graph image declarations for Main/Post paths.
- Left `attachments` creation at `sample_count=1` (behavior‑neutral). Resolve path will be introduced when Main adopts graph targets.

Validation
- CI green; no behavior change with default `sample_count=1`.

---

## Phase‑Three — PR 50: Passes consume graph views (behavior‑neutral)

Changes
- Updated post pass helpers (`pass_ao/ssgi/ssr/bloom`) to accept an explicit render target `&TextureView` and bound them in exec closures via `ctx.view_color(handle)`.
- Ensures pass code does not hardcode `attachments.scene_view` for color outputs; depth/UI sampling remains via existing BGs for parity.

Validation
- `cargo test` and `clippy -D warnings` green; visuals unchanged.

---

## Phase‑Three — PR 51: MSAA resolve as a pass (shape only)

Changes
- Graph build now declares `hdr` (single-sample), `depth(samples)`, and, when `samples>1`, a `msaa` color. Added `ResolvePass` (reads `msaa` → writes `hdr`).
- Present/Post/Particles/UI now thread the `hdr` handle. Exec body for resolve is a no‑op for now (aliasing path maps both handles to the attachment view), keeping visuals identical.

Validation
- `xtask ci` green; toggling sample count changes declared pass layout without affecting visuals.

---

## Phase‑Three — PR 53: Present recovery counter

Changes
- Added `Renderer.present_recoveries` and increment it on `SurfaceError::Lost/Outdated` in `PresentPass`; reuses existing resize/reconfigure.

Validation
- CI green; counter increments on forced resize/lost (manual local). Perf HUD wiring will follow in later perf plumbing PRs.
