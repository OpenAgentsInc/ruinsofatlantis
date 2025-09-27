Title: Lightning M2 — Indirect Capture Atlas (Tile‑based surface captures)

Goal
- Provide robust lighting data for rays that leave the screen by capturing per‑mesh surface attributes from representative viewpoints into a runtime atlas. Integrate with time‑of‑day so captures refresh progressively as sun/sky changes. Start with attribute capture (not radiance); consider radiance later when stable.

Scope
- Offline/at‑import descriptors for per‑mesh capture tiles; runtime atlas management; recapture scheduler; sampling path at ray hits; basic debug overlays.

Planned files and module map (aligned with src/README.md)
- Assets/import
  - `src/assets/capture.rs` — Build/generate per‑mesh capture descriptors (tile directions, projection params, footprints) during import.
  - Optional CLI: `src/bin/capture_gen.rs` — One‑time/offline generator to write sidecar JSON for meshes.

- Renderer: capture system
  - `src/gfx/gi/capture/atlas.rs` — Atlas allocation, residency, and views.
  - `src/gfx/gi/capture/recapture.rs` — Budgeted recapture scheduler (distance/importance/age); TOD invalidation policies.
  - `src/gfx/gi/capture/sampling.rs` — Project ray hits to capture tile domain; nearest‑tile selection and blending with depth test.
  - `src/gfx/gi/capture/capture.wgsl` — Offscreen pass shader to render surface attributes into the atlas (not radiance initially).
  - `src/gfx/pipeline.rs` — Extend with capture pipelines and bind layouts.
  - `src/gfx/mod.rs` — Integrate capture updates into per‑frame render order.

- Debug tools
  - `src/gfx/debug/capture_viz.rs` — Visualize tile placement per mesh in scene.
  - `src/gfx/debug/atlas_inspector.rs` — On‑screen viewer/heatmap for the atlas and residency.

Connections to existing hierarchy (read src/README.md)
- `src/gfx/sky.rs` is the single authority for sun/sky/TOD; expose change deltas for recapture invalidation.
- `src/gfx/types.rs` may need small structs for capture metadata UBOs; document std140 padding if used.
- Update `src/README.md` under `assets/` and `gfx/` to describe the new capture pipeline and tools.

Acceptance criteria
- Meshes produce capture descriptors (either at import or via the CLI) with small tile counts and reasonable coverage.
- A runtime atlas updates a limited number of tiles per frame near the camera; residency and heatmaps are visible in overlays.
- GI/SSR sampling can query attributes from the atlas when screen‑space rays miss, improving stability relative to Phase 1.
- Captures gradually refresh as TOD changes (sun angle/turbidity deltas) without large hitches.
- Stress flythrough: “tiles recaptured per second” stable within ±10%; no >4 ms spikes at 1080p on the target tier.
- Visual: turning the sun 30° over 5 s shows smooth updates without wholesale popping.

Detailed tasks
- Descriptor generation
  - Implement `src/assets/capture.rs` to compute area‑weighted k‑means over mesh normals (k≈6–16) for tile directions; add a dominant‑plane heuristic for large planar meshes.
  - Use per‑tile orthographic projection bounds derived from the mesh’s AABB projected onto the tile plane; store projection params.
  - Decide persistence format: in‑memory only initially; optional sidecar via `src/bin/capture_gen.rs` writing JSON under `data/`.
  - Tests: parameterization maps surface points to tile domain with low distortion on primitives.

- Atlas + recapture
  - Implement `atlas.rs` with fixed‑size page allocator and attribute targets (see formats below).
  - Implement `recapture.rs` with budgets (tiles/frame, bytes/frame) and priority scoring:
    - Priority = w_d·distance^-2 + w_a·age + w_m·mesh_motion + w_t·TOD_delta
  - Implement `capture.wgsl` to render G‑Buffer‑like attributes (albedo, normal oct, rough/metal, emissive, depth) from the tile’s view into the atlas.

- Sampling
  - Implement `sampling.rs`: score tiles by `wN·dot(Nhit,dir)^p + wP·proximity` (p≈4); pick best, project hit to tile plane; depth‑test against tile depth ± thickness; if miss, try next best then fall back to BVH (M3).
  - Integrate into `gi/ssgi.rs` and `reflections/ssr.rs` miss paths.

- Tooling
  - Implement `capture_viz.rs` to draw per‑mesh tile quads/directions in world; draw counts on HUD.
  - Implement `atlas_inspector.rs` to show atlas pages/mips and residency heatmap.

- Integration and config
  - Extend `LightingConfig` with atlas size, tiles per frame, capture mode (attributes vs radiance), and invalidation thresholds for sun/sky.
  - Update `src/README.md` for all new modules and tools.

Suggested formats (attributes‑first)
- A0: `RGBA16F`  → albedo.rgb + emissive (alpha) or spare channel
- A1: `RG16Snorm` (normal oct) + `RG8Unorm` (roughness, metalness) packed
- Depth: `R32Float` (tile‑space linear depth)
- Indirection table: `mesh_id : [tile_id → {page, uvRect, dir, plane}]`

Tests
- Assets: property tests for projection to tile domain; coverage on simple meshes (cube/plane).
- Runtime: deterministic budgeted recapture order with fixed camera path; assert steady residency counts.
 - Sampling: verify tile depth tests accept expected hits within thickness bias; reject outside bounds; fallback proceeds to BVH.

Out of scope
- Triangle BVH/SDF tracing (Lightning M3).
- Multi‑bounce GI and denoisers (Lightning M4).

Housekeeping
- Keep docblocks at top of new modules per repo guidelines.
- If adding any crates for JSON sidecars or tooling, use `cargo add` and document rationale.
