# Renderer Audit (`crates/render_wgpu`)

Context
- Renderer is modular at the file level with many subsystems under `gfx/*`, plus a recent extraction of orchestration into `gfx/renderer/*`.
- Strengths: separation of CPU helpers (terrain, hiz), WGSL assets co-located, CPU math tests exist.
- Risk: Complex orchestration across large files with state, input, updates, and passes interleaved; resource rebuild rules are implicit.

Notable Files
- `crates/render_wgpu/src/gfx/renderer/state.rs:1` — central state container; consider narrowing fields via sub-structs per pass.
- `crates/render_wgpu/src/gfx/renderer/init.rs:1` — device/surface/attachment/pipeline setup; long; benefits from sub-builders and explicit lifetime docs.
- `crates/render_wgpu/src/gfx/renderer/update.rs:1` — heavy CPU updates including collision coupling.
- `crates/render_wgpu/src/gfx/renderer/passes.rs:1` and `render.rs:1` — pass orchestration.

Pain Points
- Monolithic state with many wgpu objects; resizing and rebind logic are spread out.
- Feature toggles (`enable_post_ao`, `enable_ssgi`, `enable_ssr`, `enable_bloom`) cut across resource lifetimes.
- Renderer couples directly to `collision_static` and client controller in update; creates bidirectional growth risk.

Recommendations
1) Structure: State / Resources / Passes
- State: split `Renderer` into smaller structs: `GpuCtx` (device/queue/surface/config/samplers), `Attachments` (color/depth/scene_read + sizes), `Pipelines` (all pipelines + BGLs), `SceneBuffers` (globals/instances/palettes), `Counts/Stats`.
- Passes: each pass owns its bindgroups; `Attachments` produces the required views.
- Resize: single `rebuild_attachments(new_size)` with idempotent inputs; `Pipelines` only depend on static formats or swapchain formats.

2) Minimal Frame Graph
- Introduce a tiny frame-graph descriptor (nodes/edges) to encode read/write dependencies among attachments; disallow sampling from write-targets in the same frame by construction.
- Generate encoders from the graph; simplify `render()` control flow.

3) Extract Update Pipelines
- Move client/controller updates and collision resolve into a `client_core` or `gameplay_client` crate where the renderer consumes only derived transforms/instances.
- Use a narrow trait in the renderer for “scene inputs” (instances, transforms, palettes, toggles).
- Remove renderer‑local ability cooldown tracking. Input gating and HUD timers must consume ability state from `client_core`/server and derive durations from `SpecDb`; do not hardcode values (e.g., Fire Bolt’s cooldown) inside renderer modules.

4) BGL/Pipeline Registry
- Centralize bind group layout creation and keep typed keys per pass to avoid mismatched BG usage after resizes.
- Document all WGSL entry points and expected layouts in module-level docs.

5) Testing Additions
- CPU-only tests that build terrain meshes, NPC instances, and wizard palettes → hash results.
- Orchestration tests that toggle passes and verify resource invariants (no read-after-write on same view; proper `scene_read` usage).
- WGSL validation in CI via Naga.

6) Naming and File Hygiene
- Rename large transitional files (`new_core_*`, `render_core_*`) to stable subsystem names; split if >1K LOC.
- Add module docs on data flow at the top of each file (inputs/outputs/invariants).

7) Asset Path Policy
- Unify path resolution using `shared/assets` helpers instead of ad-hoc `asset_path` branches; provide workspace-first policy with tests.

Incremental Plan (safe steps)
- Week 1: Introduce `Attachments` and `Pipelines` structs and move only construction and resize concerns; keep the rest intact. Add tests for resize idempotency.
- Week 2: Extract update logic (controller/collision) behind a trait; renderer consumes scene inputs. Add CPU hashing tests.
- Week 3: Add minimal frame-graph (static DAG) for scene → post → present; enforce resource access rules.
- Week 4: Rename and split files; add WGSL CI validation and BGL registry.
