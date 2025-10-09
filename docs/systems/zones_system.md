# Zones System

Authoring lives in `/data/zones/<slug>`. Core file: `manifest.json`.

- `ZoneManifest` declares ids, terrain, vegetation, weather, and initial TOD.
- Loader: `data_runtime::zone::load_zone_manifest`.
- Runtime: renderer reads the manifest and builds terrain/instances.

## Implementation Status (v0)

What’s wired up in the repo today:

- Zone Selection at Boot
  - Native and Web builds accept a zone slug via `ROA_ZONE` or URL query `?zone=<slug>`.
  - Platform sets the zone before first frame; no renderer gameplay branches are required.

- Snapshot Loader (tolerant)
  - `data_runtime::zone_snapshot` loads baked files from `packs/zones/<slug>/snapshot.v1/`.
  - Reads `meta.json` and optional blobs (e.g., `colliders.bin`, `colliders_index.bin`). Missing files are tolerated so formats can evolve.

- Client Presentation
  - `client_core::zone_client::ZonePresentation::load(slug)` loads a snapshot root for the client.
  - Renderer exposes `set_zone_batches(...)` and checks `has_zone_batches()` to decide whether to draw legacy, hardcoded static content. When a Zone is present, the renderer draws the Zone static world + replicated actors only.

- Bake (tooling)
  - `tools/zone-bake` includes a minimal library API used by tests to emit a small `snapshot.v1` (terrain/trees/meta/colliders). It’s intentionally lightweight and deterministic.

- Tests
  - Schema: serde‑level validation for a minimal `scene.json` structure (`deny_unknown_fields`).
  - Client: zone loader reads a temp snapshot root.
  - Renderer policy: headless unit tests verify content selection without GPU.

## Next Steps

Data & Schemas
- Define a first `.roazone` (or `scene.json`) authoring schema with JSON Schema + serde. Include instances, logic (spawns/triggers), waypoints, and links.
- Add a `schemas/` directory and programmatic validators (used by `xtask` and CI) for all example scenes in `data/zones/**`.

Bake Pipeline
- Expand `tools/zone-bake` to output full `snapshot.v1/*`:
  - `instances.bin` (static mesh instances), `clusters.bin` (culling grid), `colliders.bin` (+ index), optional navmesh, and `logic.bin`.
  - Determinism checks: enable a fast determinism test in CI with reduced sizes (smaller terrain, fewer instances) and keep a heavier one `#[ignore]` for local.

Server Integration
- Introduce `server_core::zones` with a `ZoneRegistry` and `boot_with_zone(world, slug, specs)` that:
  - Mounts static instances/colliders/navmesh from the snapshot.
  - Applies zone logic to spawn initial actors. Example: `cc_demo` spawns only the PC; a combat zone spawns encounter NPCs.
  - Drives authoritative replication (client only renders what is replicated).

Client & Renderer
- Implement CPU decode of baked instance/cluster formats in `client_core::zone_client` and upload to GPU via a new `gfx/zone_batches` path.
- Remove remaining legacy static draws once Zones provide equivalent content everywhere.
- Keep renderer free of gameplay nouns; treat Zones + replication as the single source of visual truth.

Site & Routing
- Ensure the website routes for demos append `?zone=<slug>` (or set `ROA_ZONE`) so the live WASM build boots the intended Zone without code changes.

Tests & CI
- Golden checks for snapshot meta (counts/bounds) for sample zones to catch drift.
- Grep guards in CI to forbid reintroducing demo branches into the renderer (e.g., `cc_demo`, `demo_mode`).

Assets & Safety
- If large GLBs need to live in-repo, use Git LFS. Avoid leaking absolute local paths in logs when loading assets on WASM.

Docs
- Expand this document with the `.roazone` schema once it’s finalized.
- Consider updating GDD references pointing to `docs/world/zones.md` to this page (`docs/systems/zones_system.md`).
