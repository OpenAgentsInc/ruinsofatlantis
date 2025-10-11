# Zones System (Authoring, Bake, Runtime)

This document is the canonical technical spec for Zones: how we author them, bake deterministic snapshots, and load/present them at runtime on client and server. It consolidates and supersedes older notes in `docs/systems/zone_builder.md` and `docs/systems/zones_system.md`.

---

## 1) Purpose & North Star

Zones are first‑class, data‑driven content units. Authors create a Zone (instances, logic, terrain), bake it deterministically, and run it without renderer branches.

- Today (v0.1–v0.2): minimal authoring and a tolerant snapshot loader; renderer uploads static batches when a Zone is present.
- Soon (v0.3+): richer authoring (scatter, triggers/spawns), minimal navmesh, CI schema checks.
- Later (v1.x): DM/live‑ops tools and advanced editing (terrain paint/heightmaps, prefab kits).

Non‑negotiables
- Data‑driven, ECS‑compliant (components + data; no hardcoded archetypes).
- Server‑authoritative simulation; renderer remains presentation‑only.
- Deterministic pipeline: same inputs + seeds → same outputs.

---

## 2) Authoring Overview

Authoring lives under `data/zones/<slug>/` with a manifest and scene authoring data.

- Manifest (`manifest.json`): ids, plane, size, seeds, environment defaults (TOD/weather), spawn tables, connectors.
- Scene (`scene.json` or `.roazone`): instances (static meshes), logic (spawns/triggers), waypoints, links, editor grid metadata (optional).

Grid/Tiles (optional, editor convenience)
- An optional `grid` block allows tile‑brush workflows; it serializes to instances in the baked snapshot.

---

## 3) Bake Pipeline (deterministic)

Tooling: `tools/zone-bake`
- Inputs: `data/zones/<slug>/manifest.json`, scene authoring data, assets.
- Output: `packs/zones/<slug>/snapshot.v1/` containing:
  - `instances.bin` — static instancing
  - `clusters.bin` — culling grid/partitions
  - `colliders.bin`, `colliders_index.bin` — physics colliders
  - `meta.json` — optional metadata (bounds, display name, ids)

Determinism
- Stable ordering and seeded generation; CI can run reduced‑size golden checks.

---

## 4) Runtime Loading & Presentation

Snapshot loader (tolerant)
- `data_runtime::zone_snapshot` reads any present files under `snapshot.v1/` and tolerates missing ones so formats can evolve.

Client presentation
- `client_core::zone_client::ZonePresentation::load(slug)` locates and validates a snapshot.
- `render_wgpu::gfx::zone_batches::upload_zone_batches(&Renderer, &ZonePresentation)` returns a `GpuZoneBatches` handle.
- Renderer integration: `Renderer::set_zone_batches(Some(GpuZoneBatches))` attaches the zone; when present, renderer draws Zone static + replicated actors and skips legacy hardcoded demo content.

Server integration (initial)
- Demo server checks the selected zone and spawns only encounter actors when appropriate. A future `server_core::zones` will mount colliders/navmesh and apply zone logic to spawn initial actors authoritatively.

Selecting a zone
- Native: `ROA_ZONE=<slug> cargo run -p platform_winit`
- Web: append `?zone=<slug>` to the URL. Examples:
  - Live site: `/play?zone=cc_demo`
  - Local dev (Trunk): `http://127.0.0.1:8080/?zone=cc_demo`
- Back‑compat: `RA_ZONE` is still accepted, but `ROA_ZONE` is canonical.

---

## 5) Status & Next Steps

Implementation status (v0)
- Zone selection at boot on native/web via env or URL query.
- Tolerant snapshot loader in `data_runtime`.
- Client presentation + renderer capability flag (`has_zone_batches`).
- Minimal bake library used by tests; deterministic intent.

Next steps
- Define a first `.roazone` schema (JSON Schema + serde) for instances, logic, links.
- Expand `zone-bake` to emit full `snapshot.v1/*` (instances/clusters/colliders/navmesh/logic) and add CI determinism checks.
- Add `server_core::zones` with `ZoneRegistry` + `boot_with_zone(...)` to mount snapshot content and apply logic.
- Implement CPU decode of baked instance/cluster formats and real GPU uploads; remove remaining legacy static draws once Zones are complete.
- Site routing: ensure WASM routes include `?zone=<slug>` for live demos.

---

## 6) Editor & Builder Notes (consolidated)

Two complementary creation paths share the same data model:
- Desktop Zone Editor (`tools/zone-editor`) — production authoring with a preview viewport.
- In‑game Builder Zone (`builder_sandbox`) — hands‑on creation: tile brush for assets, save new zone, “Bake & Play”.

Editor grid schema (optional)
- `grid: { cell_size, width, height, tiles[] }` where each tile stores `{x,y,rot,asset_id,layer,tags}`; tiles are convenience and bake to instances.

---

## 7) Testing & CI

- Schema validation (serde + JSON Schema) for scene authoring inputs.
- Golden snapshot meta checks (counts/bounds) to catch drift.
- Headless renderer policy tests ensure no demo‑only branches when a Zone is attached.
- Grep guards in CI prevent reintroducing demo conditionals into renderer.

---

## 8) References

- Authoring: `data/zones/<slug>/`
- Bake outputs: `packs/zones/<slug>/snapshot.v1/`
- Loaders: `data_runtime::zone_snapshot`, `client_core::zone_client`
- GPU: `render_wgpu::gfx::zone_batches`

