Title: Lighting M3 — Software Ray Tracing (BVH baseline, optional mesh SDFs) + Hit Evaluation Modes

Goal
- Handle off‑screen ray hits robustly using a two‑level triangle BVH (TLAS/BLAS) as baseline, with an optional signed‑distance field (SDF) path for detail rays. Support two hit evaluation modes: fast (sample from capture atlas) and quality (evaluate BRDF at the true hit), selectable per material and overridable per view.

Scope
- Add CPU BVH build + traversal API (GPU traversal optional later), optional SDF tracing behind a Cargo feature, and per‑material reflection toggle. Integrate miss paths from SSR/SSGI to call BVH/SDF tracing.

Planned files and module map (aligned with src/README.md)
- BVH
  - `src/gfx/rt/bvh/build.rs` — Build BLAS per mesh and a TLAS over instances; LBVH (Morton) + SAH refit pass.
  - `src/gfx/rt/bvh/traverse.rs` — Ray traversal API; returns closest hit (triangle id, barycentrics, normal), supports masks and backface/alpha cutout options.

- Optional mesh SDFs (feature: `mesh_sdf`)
  - `src/gfx/rt/sdf/build.rs` — Generate coarse SDF volumes per mesh (voxel or sparse).
  - `src/gfx/rt/sdf/trace.rs` — Sphere tracing; detail rays; fall back to BVH if needed.

- Reflections and GI integration
  - `src/gfx/reflections/rt.rs` — Reflection rays using BVH/SDF when SSR misses; choose hit mode (fast/quality).
  - `src/gfx/gi/rt.rs` — Diffuse GI rays using BVH/SDF when SSGI misses; default to fast mode.

- Materials/config
  - `src/gfx/material.rs` — Add per‑material toggle for reflection hit mode: `force_quality_reflections: bool`.
  - `src/gfx/mod.rs` — Extend `LightingConfig` with toggles for BVH on/off, SDF on/off, and per‑view defaults for hit mode.

Connections to existing hierarchy (read src/README.md)
- Integrate with `src/gfx/gi/capture/sampling.rs` (Lighting M2) for fast hit fetch from the capture atlas.
- Keep renderer wiring in `src/gfx/mod.rs` and pipeline creation in `src/gfx/pipeline.rs`.
- Update `src/README.md` under `gfx/` to document `rt/` modules and reflection/gi RT paths.

Acceptance criteria
- Two‑level BVH builds on current scene assets; traversal returns correct closest hits on test geometry.
- BVH build time for the current scene ≤ 50 ms on target dev hardware; per‑frame TLAS refit exists for moving instances (coarse is OK).
- SSR/SSGI miss paths fall back to BVH/SDF tracing; visual stability improves for off‑screen content.
- Per‑material toggle switches reflection evaluation between fast (atlas sample) and quality (BRDF at hit); per‑view override works.
- Feature flag `mesh_sdf` compiles; when enabled, detail rays can use SDF tracing on meshes flagged `detail_reflection = true`.

Detailed tasks
- BVH
  - Implement AABB generation from meshes and instance transforms.
  - Build BLAS with LBVH; run a top‑down SAH refit for quality; build a TLAS over instances.
  - Choose a node format friendly to future GPU traversal (SoA AABBs + child indices; ~32‑byte nodes).
  - Provide traversal API:
    
    ```rust
    pub struct Ray { origin: Vec3, dir: Vec3, t_min: f32, t_max: f32, mask: u32 }
    pub struct Hit { t: f32, tri_id: u32, bary: Vec2, geom_n: Vec3, inst_id: u32, mesh_id: u32 }
    pub fn trace_any(ray: &Ray) -> Option<Hit>;
    ```
  - Support backface culling and alpha cutout in an any‑hit style callback for materials.
  - Tests: AABB intersection, tree invariants, hit ordering on analytic scenes.

- SDF (optional)
  - Implement coarse SDF bake for static meshes; store in compressed 3D textures or CPU arrays.
  - Sphere tracing with step caps and normal estimation via gradient; fallback to BVH when gradient magnitude is poor.
  - Tests: sign consistency on simple shapes; convergence within step budget.

- Reflection/GI integration
  - `reflections/rt.rs`: generate rays from G‑Buffer normals and view; handle roughness distribution.
  - `gi/rt.rs`: cosine‑weighted rays; low spp; temporal accumulate.
  - Miss path order: try SSR/SSGI -> BVH/SDF -> environment fallback.

- Config and materials
  - Extend `LightingConfig` with RT toggles and budgets (max rays, max steps, sdf detail).
  - In `material.rs`, add `force_quality_reflections` and expose a default; add a view override for cutscenes.
  - Update `src/README.md` accordingly.

Suggested data formats
- BVH nodes in SSBO‑friendly structs (even if CPU for now) to ease future GPU traversal.
- SDFs in R16F volumes per mesh (optional feature).

Tests
- BVH unit tests under `rt/bvh/*` for intersection math and traversal determinism.
- SDF unit tests under `rt/sdf/*` guarded by feature.
- Cornell‑box‑like integration test validates barycentrics and normal orientation (handedness consistency).

Out of scope
- Multi‑bounce GI and denoisers (Lighting M4).

Housekeeping
- If a helper crate is introduced (e.g., `bitvec` for SDF occupancy), add via `cargo add` and document.
- Keep docblocks and rustdoc complete; maintain compiling state with `clippy` clean.
