# Large Files Audit — Structure, Roles, and Refactor Suggestions

Context
- Read: docs/architecture/ECS_ARCHITECTURE_GUIDE.md and crates/README.md to align with layering and ownership rules.
- Goal: Identify source files ≥1000 LOC, summarize their responsibilities, and suggest refactors that improve cohesion, layering, and testability without changing behavior.

Scope (code files ≥1000 LOC)
- crates/render_wgpu/src/gfx/mod.rs (~4801)
- crates/render_wgpu/src/gfx/renderer/update.rs (~3394)
- crates/render_wgpu/src/gfx/renderer/render.rs (~2227)
- crates/render_wgpu/src/gfx/renderer/init.rs (~2217)
- crates/render_wgpu/src/gfx/ui.rs (~2127)
- tools/model-viewer/src/main.rs (~2088)
- crates/platform_winit/src/lib.rs (~1777)
- crates/server_core/src/ecs/schedule.rs (~1550)
- crates/server_core/src/lib.rs (~1389)
- crates/render_wgpu/src/gfx/pipeline.rs (~1302)
- crates/net_core/src/snapshot.rs (~1024)
- crates/render_wgpu/src/gfx/vox_onepath.rs (~1017)

Notes
- Vendor/asset binaries (GLB/PNG/WASM) excluded.
- Suggestions emphasize small, focused modules, explicit APIs, and adherence to ECS layering.

## render_wgpu::gfx::mod.rs
- Role: Central renderer module owning GPU device/surface, attachments, pipelines, bind group layouts, scene buffers, terrain, sky, particles, instancing, worldsmithing ghost, and draw/update glue.
- Risks
  - God-struct (`Renderer`) with many orthogonal concerns (device/surface, pass setup, resource lifetime, draw lists, CPU updates, upload helpers).
  - High coupling between CPU-side update logic and draw code; many fields stored to reconstruct state across passes.
  - Resize/pipeline rebuild logic interleaved with draw code.
- Refactor suggestions
  - Extract cohesive subsystems:
    - Device/surface + attachments → `renderer::device.rs` (`GpuCtx`, `Attachments` already exists; expand it).
    - Pipelines + BGL registry → `renderer::pipelines/*` per pass (terrain, instanced, sky, post, particles). Provide builders returning typed handles.
    - Framegraph orchestration → `renderer::graph.rs` to schedule passes explicitly (align with docs/gdd/11-technical/graphics/frame-graph.md). Keep pass IO explicit and validated.
    - Draw lists/builders → `renderer::drawlists.rs` (group by pipeline/material; minimize state changes).
    - CPU update paths remain in `renderer::update.rs` with narrow mut APIs on `Renderer`.
  - Encapsulate bind group layouts into typed wrappers (e.g., `PresentLayouts`, `PostAoLayouts`) to avoid passing many raw BGLs.
  - Narrow `Renderer` fields behind sub-structs (`Pipelines`, `Samplers`, `SceneBuffers`).
  - Move constants/config to a small `renderer::config.rs` (e.g., toggles, thresholds).
  - Add lightweight unit tests for pass IO invariants and drawlist grouping (CPU-only).

## render_wgpu::gfx::renderer::update.rs
- Role: CPU-side update helpers (projectile VFX, ruin AABBs, small RNG helpers, builder ghost plumbing, destructible demo hooks when enabled).
- Risks
  - Mixed responsibilities (math helpers, demo-only destructible paths, scene mutation helpers) living alongside generic update utilities.
  - Feature-gated demo code increases cognitive load; some helpers are generic enough to move to a math/util module.
- Refactor suggestions
  - Split by concern: `update_projectiles.rs`, `update_builder.rs`, `update_destructibles_demo.rs` (behind feature), `math.rs` for generic helpers.
  - Keep `Renderer` methods minimal; push pure helpers to free functions/modules for easier unit tests.
  - Ensure all demo/destructible code stays behind features and not compiled by default.

## render_wgpu::gfx::renderer::render.rs
- Role: Main per-frame render function; builds passes, sets pipelines, issues draws; integrates UI/HUD and post.
- Risks
  - Large function bodies with interleaved pass setup/draw calls; harder to reason about ordering and state lifetimes.
- Refactor suggestions
  - Adopt a simple framegraph executor (as per docs) and split pass recorders: `pass_sky`, `pass_main`, `pass_post`, `pass_present`.
  - Consolidate repeated setup code (bind groups, common vertex buffers).
  - Group draws by material/pipeline; prebuild draw lists outside the encoder.

## render_wgpu::gfx::renderer::init.rs
- Role: Device/surface creation, adapter selection, surface config, attachment/pipeline creation.
- Risks
  - Intermixes capability probing, surface decisions, and pipeline creation.
- Refactor suggestions
  - `GpuCtx::new()` (adapter/surface/device/queue) + `SurfaceCtx::configure()`; pass capabilities down.
  - Per-pipeline builders under `pipelines/*` called from a small `build_pipelines(&GpuCtx, &Attachments)`.
  - Centralize sampler creation.

## render_wgpu::gfx::ui.rs
- Role: HUD/UI building and draw batching.
- Risks
  - UI logic intertwined with renderer choices and input toggles.
- Refactor suggestions
  - Keep UI dataflow in `ux_hud` (logic/state). Restrict this module to GPU vertex building and draw calls from flattened HUD data.
  - Split overlays (perf, help, hotbar) into small functions for clarity; unit test vertex counts/build.

## tools/model-viewer::main.rs
- Role: Standalone GLTF/GLB viewer; CLI parsing, loader, UI, and render loop in one file.
- Risks
  - Single-file tool with CLI, IO, UI, and rendering blows past 2k LOC; tough to maintain.
- Refactor suggestions
  - Split: `cli.rs` (clap), `app.rs` (state), `viewer.rs` (render), `panels.rs` (UI), `loader.rs` (asset load/merge). Keep `main.rs` at bootstrap.
  - Move logging and snapshot helpers into `utils.rs`; add small unit tests for loader decisions (dominant skin, rebasing bounds).

## platform_winit::lib.rs
- Role: Application handler, window/surface lifecycle, input mapping, zone picker, builder overlay, replication wiring.
- Risks
  - Broad set of responsibilities in one file; heavy feature gating; event loop code becomes cluttered.
- Refactor suggestions
  - Split: `app.rs` (ApplicationHandler), `input.rs` (keybinds → intents), `picker.rs` (zone picker), `builder_overlay.rs`, `replication.rs` (loopback wiring), `telemetry.rs` (init).
  - Keep event handlers small; move logic into helpers.

## server_core::ecs::schedule.rs
- Role: ECS schedule definition and helpers; system ordering and stage composition.
- Risks
  - Long schedule builder makes reordering/testing difficult; test hooks buried.
- Refactor suggestions
  - Declarative stage macros or small DSL for stages; expose `system_names_for_test()` for verification.
  - Split stage assembly across files: `stage_input`, `stage_ai`, `stage_move`, `stage_combat`, `stage_cleanup` with a final `build_schedule()` that composes them.

## server_core::lib.rs
- Role: Crate root + many module declarations/exports.
- Risks
  - Large `lib.rs` can hide module boundaries and confuse ownership.
- Refactor suggestions
  - Keep `lib.rs` to re-exports and high-level docs. Move inline modules to `src/<mod>.rs`.

## render_wgpu::gfx::pipeline.rs
- Role: Pipeline and bind group creation for multiple passes.
- Risks
  - All pipelines in one file increases rebuild and mental overhead; hard to reuse small pieces.
- Refactor suggestions
  - Per-pipeline modules: `pipeline/terrain.rs`, `pipeline/instanced.rs`, `pipeline/sky.rs`, `pipeline/post/{ao,ssgi,ssr,bloom}.rs` with shared `pipeline/common.rs` for layouts.
  - Return typed handles wrapping `wgpu::RenderPipeline` + layouts.

## net_core::snapshot.rs
- Role: Snapshot message structs, encode/decode, and apply scaffolding.
- Risks
  - Mixed responsibilities (schema, encode/decode helpers, apply tests) in one file.
- Refactor suggestions
  - Split by domain: `snapshot/actors.rs`, `snapshot/projectiles.rs`, `snapshot/destructibles.rs`, `snapshot/hud.rs`; keep shared `encode.rs`.
  - Maintain versioned structs by module; keep doc comments with wire invariants.

## render_wgpu::gfx::vox_onepath.rs
- Role: Feature-gated demo path (procedural block, carve burst, screenshot).
- Risks
  - Large demo module in core renderer tree; risks accidental coupling.
- Refactor suggestions
  - Move under `examples/` or `tools/` or keep behind a strict feature and split into `demo/*` modules; add CI guard to ensure it’s disabled by default.

---

Cross-cutting suggestions
- Apply the frame graph invariants consistently (never read-write same resource in a frame).
- Prefer typed wrappers over raw wgpu objects in public fields; encapsulate resize and rebuild paths.
- Keep client rendering strictly presentation-only per ECS guide; demo/gameplay code stays gated or migrates to tools.
- Add brief rustdoc to public structs/functions for cargo doc usefulness.
- Incrementally extract modules; avoid large rewrites. Start with pipelines and render passes where boundaries are clearest.

Validation
- After refactors, ensure `cargo xtask ci` remains green (fmt, clippy -D warnings, WGSL validation, tests, schema checks).
- Add small unit tests for CPU-only logic (drawlist grouping, vertex generation, math helpers, schedule names).

