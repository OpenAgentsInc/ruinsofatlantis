# Issue 101 — 3D Zone Builder (Editor + Bake Pipeline)

Purpose
- Deliver a desktop editor that lets designers assemble 3D zones visually (place assets, author triggers/quests, tune TOD/weather), then bake them into deterministic, performant, playable snapshots consumed by the runtime.
- Reuse the existing renderer/runtime crates so WYSIWYG closely matches in‑game; keep schemas/versioning in `data_runtime` and extend `tools/zone-bake` for baking.

Context
- Downloaded kit observed: `~/Downloads/Stylized Nature MegaKit[Standard] (1)/{glTF, FBX, OBJ, Textures}` with many GLTFs (trees, rocks, grass, paths) and PNG textures. This will seed the initial environment library (import as GLTF to avoid format churn).
- Current repo: zones have `manifest.json` with TOD controls; `tools/zone-bake` bakes terrain + trees; renderer supports instancing and GLTF via `roa_assets`.

Out of Scope (initial)
- Navmesh quality tooling (advanced parameters/visualization beyond basic generation), multi‑user collaboration, networked editing, streaming/mega‑world editing, lighting bake/global illumination bake.

---

## Scope (repo‑aware)

New
- `tools/zone-editor` — desktop editor binary (winit + render_wgpu viewport; simple UI overlay; non‑interactive in CI).
- `crates/data_runtime/schemas/zone.scene.schema.json` — scene schema (instances, layers, logic volumes, spawn points, links).
- `crates/data_runtime/src/zone_scene.rs` — serde types + loader/validator for scene files.
- `crates/ecs_core/src/components/zone.rs` — ECS tags/components for placed content (StaticInstance, LogicVolume, SpawnPoint, Waypoint, TagSet).
- `docs/systems/zone_builder.md` — design doc for editor UX/data flow (kept current with changes).

Changes
- `tools/zone-bake` — extend to consume `zone.scene.json` and output `packs/zones/<slug>/snapshot.v1/*` with instance clusters, culling grid, colliders, and optional navmesh.
- `crates/render_wgpu` — add a lightweight scene preview path (CPU scene to GPU instances) behind a feature flag for the editor.
- `data/zones/<slug>/manifest.json` — honored by editor (TOD/weather); add reference to `scene.json`.

Integration
- Asset ingestion: import GLTFs from `assets/models/**` (including the MegaKit GLTFs); track large assets via git‑lfs. Use Draco‑decompressed GLTF if needed (not observed in the kit). Use `roa_assets` conventions.

---

## User Workflow

- Open editor → choose/create zone (slug) → viewport shows sky/TOD from manifest.
- Browse assets by category (Trees/Rocks/Grass/Props) → drag into viewport.
- Position with gizmos; grid/snap; align‑to‑ground; per‑axis scaling; duplicate; multi‑select; grouping.
- Scatter brush for vegetation (jitter rotation/scale, surface mask, density/seed for determinism).
- Author logic: trigger volumes, spawn points, waypoints, region tags, door/portal links.
- Save: writes `data/zones/<slug>/scene.json` (validated against schema).
- Bake: runs `tools/zone-bake` to produce snapshot; run app to play zone (manual, not in CI).

---

## Data Model (v1)

Files
- `data/zones/<slug>/manifest.json` — existing; fields: `start_time_frac`, `start_paused`, `start_time_scale`.
- `data/zones/<slug>/scene.json` — NEW; versioned JSON validated by schema.

Scene JSON (top‑level)
- `version: string` — semantic version of scene format.
- `seed: u64` — drives deterministic scatter and ordering.
- `layers: [string]` — names for filtering/visibility.
- `instances: [{ id, asset_id, transform { t,r,s }, layer, tags: [string], flags: { static:true, lod?:{screen_area:[]}}, user?:{k:v}}]` — static/dynamic props placed.
- `logic: { triggers:[{id, shape, transform, layer, tags}], spawns:[{id, transform, tags}], waypoints:[{id, transform}], links:[{from,to,kind}] }`.
- `terrain?: { ref?:string }` — optional pointer to height/mesh asset or baked terrain params.

Schema location and versioning
- `crates/data_runtime/schemas/zone.scene.schema.json`; changes go through `xtask schema-check` and bump `version`.

---

## Editor Architecture

- Binary: `tools/zone-editor` (desktop, winit 0.30; uses `render_wgpu` for viewport to match runtime rendering). Keep tool UI minimal and fast.
- UI overlay: simple panels for Asset Browser, Outliner, Inspector, Tools; follow Input Policy (avoid F‑keys).
- Viewport: camera nav (orbit/fly), grid ground, snapping, gizmos (move/rotate/scale), selection highlights.
- Asset registry: scan `assets/models/**` for GLTF; categorize via folder or sidecar JSON (`assets/models/_catalog.json`) with tags/categories.
- Serialization: `data_runtime` types; read/write `scene.json`; validate against JSON Schema.
- Determinism: editor uses fixed seed per scene; scatter/placement ops record parameters; bake replays ops deterministically.

Feature flags
- Editor‑only renderer hooks under `render_wgpu` feature `editor_preview`.
- Avoid shipping tool‑only code in release runtime builds.

---

## Bake Pipeline (extended)

Inputs
- `manifest.json`, `scene.json`, referenced GLTF assets, terrain data.

Outputs (under `packs/zones/<slug>/snapshot.v1/*`)
- `instances.bin` — deduped asset IDs + per‑cluster transforms (quantized where safe).
- `clusters.bin` — spatial bins for frustum/occlusion culling; instance → cluster mapping.
- `colliders.bin` — simplified collider meshes per asset with world transforms; optional convex hulls.
- `logic.bin` — triggers/spawns/waypoints serialized for `sim_core`.
- `navmesh.bin` — optional coarse navmesh generated from terrain + static meshes.
- `meta.json` — versioning, bounds, seeds, counts, content hash.

Processing steps
- Instance dedupe + atlas friendly sort; instance clustering by cell size; build coarse BVH/vox grid for culling budgets.
- Collider extraction: per‑asset collider (authorable type: triangle/convex/none) → world transforms.
- Logic export: shapes to baked volumes (AABB/OBB/spheres/poly volumes).
- Optional navmesh: initial stub via heightfield raster + region; gate under feature until tuned.

Tests (CPU/headless)
- Deterministic scatter replay test (seeded params → stable hashes).
- Instance clustering grid unit tests.
- Scene load/save round‑trip equality (ignoring float epsilon).
- Schema validation for sample scenes in `data/zones/**` via `xtask schema-check`.

---

## Controls & UX (initial)

- Camera: `RightMouse+WASD` fly; `Alt+LeftMouse` orbit; `Scroll` zoom.
- Tools: `W/E/R` move/rotate/scale; `G` toggle grid; `X` delete; `D` duplicate; `B` scatter brush.
- Snapping: `Shift` = precision toggle; `1/2/3` set snap (pos/rot/scale) magnitudes; avoid F‑keys.
- Overlays: bounds, IDs, layer filters, perf panel (P), orbit (O), HUD toggle (H) consistent with app.

---

## Milestones & Deliverables

M0 — Schema + Viewer (1–2 wk)
- Schema + serde types; load/validate `scene.json`.
- Editor viewport renders existing scene via `render_wgpu` (no editing yet).
- Asset catalog minimal; TOD/sky reflects manifest.

M1 — Core Editing (2–3 wk)
- Selection, gizmos, grid/snap, duplicate/delete, save/load.
- Asset Browser drag‑drop; Outliner; Inspector for transform/layer/tags.

M2 — Scatter + Layers (2 wk)
- Brush placement with deterministic seed; jitter params; layer filters/toggles.
- Simple grouping and prefab duplication.

M3 — Bake v1 (2–3 wk)
- Extend `zone-bake` for instances/clusters/colliders/logic export; pack outputs.
- Runtime loads new snapshot; zone is playable with authored placements.

M4 — Logic & Spawns (2 wk)
- Triggers (enter/exit), spawns, waypoints, links; minimal quest script hook in `sim_core` (e.g., trigger → spawn wave or toggle door).

M5 — Polish & Docs (1 wk)
- Undo/redo stack; selection sets; asset tagging; docs in `docs/systems/zone_builder.md`; update `src/README.md` for inputs.

---

## Tasks (repo‑aware)

Data & Schemas
- [ ] Add `crates/data_runtime/schemas/zone.scene.schema.json`; wire to `xtask schema-check`.
- [ ] Implement `crates/data_runtime/src/zone_scene.rs` with `serde` + validation helpers.

Editor Tool
- [ ] Scaffold `tools/zone-editor` (winit 0.30) using `render_wgpu` viewport; minimal overlay UI.
- [ ] Asset catalog scanning from `assets/models/**` with categories/tags; optional `_catalog.json`.
- [ ] Selection/gizmos/grid/snap; save/load `scene.json`; TOD controls honoring manifest.
- [ ] Scatter brush with seeded RNG; record operation parameters for deterministic replay.
- [ ] Follow input policy (avoid F‑keys); document in `src/README.md`.

Bake
- [ ] Extend `tools/zone-bake` to ingest `scene.json`; emit `instances.bin`, `clusters.bin`, `colliders.bin`, `logic.bin`, `meta.json`.
- [ ] Add instance clustering + culling grid builders (CPU‑only tests included).
- [ ] Export colliders per asset with authorable collider type.

Runtime Integration
- [ ] `render_wgpu` feature `editor_preview`: CPU→GPU instance path for editor; no impact on release builds.
- [ ] Load new snapshot formats at runtime; scene assembly (`ecs_core`) consumes baked data.

Docs & Hygiene
- [ ] `docs/systems/zone_builder.md` written and maintained.
- [ ] Update `src/README.md` with inputs and editor reference.
- [ ] Track large binaries via git‑lfs; do not commit raw kit downloads.
- [ ] Use Cargo tooling (`cargo add/rm/upgrade`) for new deps.

---

## Acceptance Criteria

- Editor opens zones, displays TOD/sky from manifest, and renders placed instances matching runtime visuals.
- Designers can place, move, rotate, scale, duplicate, delete; use grid/snap and scatter brush with deterministic results.
- Saving produces a schema‑valid `scene.json`; `xtask schema-check` passes.
- Baking outputs snapshot files; the runtime loads them and renders a playable zone (no regressions in budgets).
- Controls follow policy (no F‑keys); `src/README.md` updated.
- CI: `cargo xtask ci` passes locally, including schema validation and unit tests for clustering/determinism.

---

## Risks & Mitigations

- UI stack choice: keep overlay minimal; prefer simple panels to avoid heavy dependencies; isolate under `tools/zone-editor`.
- Performance parity: use shared renderer for viewport; introduce `editor_preview` feature to avoid shipping tool code in runtime.
- Asset licensing/size: import only required GLTFs into `assets/models/` and track via git‑lfs; retain upstream license files in `NOTICE` as needed.
- Determinism: centralize RNG seeding and record scatter params; add golden hashes in tests.

---

## Labels & Dependencies

- Labels: `area:tools` `area:data` `area:renderer` `type:feature` `docs-needed`
- Depends on: none strictly, but integrates with `tools/zone-bake`, `render_wgpu`, `data_runtime`, `ecs_core`.

