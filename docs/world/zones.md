# Ruins of Atlantis — **Zone Builder v0.1**

*A unified spec for creating, editing, baking, and playing zones (today), with a clear runway to DM/live‑ops and advanced terrain (tomorrow).*

---

## 0) Purpose & North Star

We’re formalizing **Zones** as first‑class, data‑driven content units and delivering a **Builder** that can author them quickly:

* **Today (v0.1–v0.2):** a simple, **tile‑based** placement tool for trees/props inside a dedicated **Builder Zone** entrypoint and the desktop **Zone Editor**. Designers can place a handful of models, save a new zone (`scene.json`), bake a snapshot, and play it immediately—**no renderer branches**.
* **Soon (v0.3+):** expand to scatter, triggers/spawns, and minimal navmesh.
* **Later (v1.x):** add **live Dungeon Master** (DM) orchestration and more advanced editing (height‑maps, terrain paint, prefab kits).

**Design influences** we are borrowing consciously:

* **Tile painting + approachable asset palette** (Neverwinter Nights, Grimrock).
* **Live GM/DM runtime** (NWN DM client, Divinity OS2 Game Master Mode).
* **In‑game, shareable dungeons/campaigns** (Solasta Dungeon Maker; “gadgets”/triggers approach).

---

## 1) Principles (non‑negotiable)

1. **Data‑driven, ECS‑compliant:** Zones and gameplay are **components + data**; no archetype/name branches in systems.
2. **Server‑authoritative:** Builder actions mutate **authoring data**, not client gameplay logic; runtime simulation remains server‑side.
3. **Renderer is presentation‑only:** The renderer draws **(Zone static instances) + (replicated actors)**; it never contains “demo” or “builder” conditionals.
4. **Deterministic pipeline:** Same inputs + seeds → same baked outputs.
5. **Two paths, one data model:**

   * **Desktop Editor** (`tools/zone-editor`) for production authoring.
   * **Builder Zone** (in‑app entrypoint) for quick, “today” creation and learning.
     Both read/write the same **scene schema** and rely on the same **bake**.

---

## 2) High‑Level Architecture (bridging Zones ↔ Builder)

```
[Designer] ──(in-app Builder Zone OR desktop Zone Editor)──► scene.json
                                              │
                                              ▼
                                    tools/zone-bake (deterministic)
                                              │
                                              ▼
                         packs/zones/<slug>/snapshot.v1/{instances,clusters,colliders,logic,meta}
                                              │
                      ┌───────────────────────┴────────────────────────┐
                      │                                                │
             [Server: ZoneRuntime + LogicSpawner]             [Client: ZonePresentation]
             spawns actors; owns truth                        uploads static batches to GPU
```

**Key change vs today:** Choosing content means **selecting a zone**; there are **no render‑time mode branches**.

---

## 3) Modes & Entry Points

### 3.1 Desktop Zone Editor (production)

* Binary: `tools/zone-editor` (winit + `render_wgpu` viewport with feature `editor_preview`).
* **Tile Brush** (v0.1): paint assets on a 2D grid (trees/rocks to start).
* Save to `data/zones/<slug>/scene.json`; run `zone-bake`; launch game with `--zone <slug>`.

### 3.2 In‑Game **Builder Zone** (today’s hands‑on)

* Zone slug: `builder_sandbox`.
* Launch paths:

  * Native: `RA_ZONE=builder_sandbox cargo run -p platform_winit`
  * Web: `/builds/builder?zone=builder_sandbox`
* UI Overlay (**Builder HUD**) exposes:

  * Asset palette (Trees → a few GLTFs), **Tile Brush**, select/move/rotate, save as **new zone**.
  * “Bake & Play” button → runs `zone-bake` → reloads runtime into the new zone.

> **Why both?** The **Builder Zone** gets creators productive today and mirrors later **DM** workflows (live/runtime). The desktop editor remains the primary, robust authoring tool.

---

## 4) Data Model (scene schema v1 additions)

We keep `scene.json` as the single source of truth. **Tile‑based editing** is a *convenience layer* that serializes to instances. Optional schema additions:

```jsonc
// crates/data_runtime/schemas/zone.scene.schema.json (new optional block)
{
  "type": "object",
  "properties": {
    "grid": {
      "type": "object",
      "properties": {
        "cell_size": { "type": "number", "minimum": 0.1 },
        "width": { "type": "integer", "minimum": 1 },
        "height": { "type": "integer", "minimum": 1 },
        "tiles": {
          "type": "array",
          "items": {
            "type": "object",
            "required": ["x","y","asset_id"],
            "properties": {
              "x": { "type": "integer" }, "y": { "type": "integer" },
              "rot": { "type": "integer", "enum": [0,90,180,270] },
              "asset_id": { "type": "string" },
              "layer": { "type": "string" },
              "tags": { "type": "array", "items": { "type": "string" } }
            }
          }
        }
      },
      "required": ["cell_size","width","height","tiles"]
    }
  }
}
```

**Bake behavior:** if `grid` is present, the baker expands it into `instances` (snap‑to‑ground, apply rotation/scale defaults). `instances[]` remains the canonical, baked input to the snapshot.

Other v1 fields unchanged (instances, logic, terrain). See earlier repo plan.

---

## 5) Bake Pipeline (extended but familiar)

* Input: `manifest.json`, `scene.json` (with optional `grid`), referenced GLTFs (via `roa_assets`).
* Output: `packs/zones/<slug>/snapshot.v1/*`
  `instances.bin`, `clusters.bin`, `colliders.bin`, `logic.bin`, `navmesh.bin? (opt)`, `meta.json`.

**New v0.1 passes:**

* **Tile→Instance expansion:** deterministic expansion order (row‑major y→x; seed applied to scatter jitter only if enabled later).
* **Instance dedupe** and **cluster binning** (unchanged).
* **Collider picks** per asset (triangle/convex/none; default by catalog).
* **Meta.counts** include `tiles_expanded`.

---

## 6) ECS & Runtime Integration

**Server (`server_core`)**

* `ZoneRegistry` + `ZoneSnapshot` (as spec’d earlier).
* `LogicSpawner` consults snapshot `logic` to spawn PCs/NPCs.

  * `builder_sandbox`: spawns only the **PlayerControlled** actor at a default or specified `SpawnPoint`.
* **No archetype branches;** all behaviors are component‑driven.

**Client (`client_core`)**

* `ZonePresentation::load(slug)` loads snapshot and prepares CPU batches.
* Renderer calls `upload_zone_batches` once; per‑frame draws: **static batches** then **replicated actors**.

**Renderer (`render_wgpu`)**

* Delete `cc_demo`/mode checks.
* Draw order: `zone_batches` → actors → projectiles → fx.
* **Animation fallback:** idle when Δspeed ≈ 0 to avoid T‑poses (data‑mapped clips).

---

## 7) Builder UX (v0.1 → v0.3)

### v0.1 — **Tile Starter**

* **Palette:** Trees (3–5 pieces), Rocks (2–3), Ground pads (optional).
* **Grid:** toggle; set `cell_size` (default 2.0m).
* **Brushes:**

  * **Paint Tile (1×1):** LMB place, RMB erase.
  * **Line/Rect Fill (bonus):** drag to place lines/areas of the selected tile.
* **Transform:** rotate 0/90/180/270 with `Q/E`, nudge with arrows (snap to grid).
* **Layers:** `Default`, `Foliage`; eye toggles for visibility.
* **Save:** write `scene.json` (validates schema). **Save As Zone…** prompts for `slug`.
* **Bake & Play:** runs baker; relaunch runtime in new zone.
* **Input policy:** `W/E/R` reserved for gizmos in editor; avoid F‑keys.

### v0.2 — **Instances & Scatter**

* Place arbitrary GLTFs (free transform, not tile bound).
* **Scatter Brush** (seeded jitter) for vegetation (deterministic), recorded parameters → reproducible bake.

### v0.3 — **Logic Primitives**

* **SpawnPoint**, **Trigger Volume** (box/sphere), **Waypoint**, **Tags**.
* Minimal **Navmesh** opt‑in bake (heightfield stub).

> **Why tile first?** Tile painting proved to be a powerful on‑ramp in **NWN** and **Grimrock**—fast results with low cognitive load.

---

## 8) Future Track — Live DM & Advanced Terrain

* **DM Control Room (v1.x):**
  Live “session” UI to **possess NPCs**, **spawn waves**, **toggle doors**, and **narrate**. Think NWN DM client / DOS2 GM Mode, built atop our replication/events. No custom code in live; DM sends **authoring ops** or **runtime events** guarded by server ACLs.
* **Terrain Heightmaps & Paint (v1.x):**
  Add `terrain{ heightmap, splat, materials[] }` to scene schema; bake to mesh + colliders; basic erosion/raise/lower tools. (Out of scope for v0.1 but data shape now.)

---

## 9) Asset Catalog (minimal to start)

* Source: `assets/models/**` (GLTF; tracked via git‑lfs for large).
* Optional `_catalog.json`:

  ```json
  { "Trees": ["env/trees/oak_A.glb","env/trees/pine_B.glb"],
    "Rocks": ["env/rocks/rock_01.glb","env/rocks/rock_02.glb"] }
  ```
* **roa_assets** conventions for IDs; catalog categories feed the Builder palette.

---

## 10) Serialization & Files

```
data/zones/<slug>/manifest.json     // TOD/weather (existing)
data/zones/<slug>/scene.json        // v1 schema (with optional grid)
packs/zones/<slug>/snapshot.v1/     // baked artifacts
```

**Versioning:** bump `scene.version` on schema change; `xtask schema-check` validates repo samples.

---

## 11) Implementation Plan (repo‑aware tasks)

**Data & Schemas**

* [ ] Extend `zone.scene.schema.json` with optional `grid` block.
* [ ] `data_runtime::zone_scene` add `GridSpec` + helpers.
* [ ] `data_runtime::zone_snapshot` finalize snapshot types (serde).

**Baker (`tools/zone-bake`)**

* [ ] Add Tile→Instance expansion pass (deterministic).
* [ ] Keep existing instance clustering/colliders; include `tiles_expanded` in meta.

**Desktop Editor (`tools/zone-editor`)**

## 12) Implementation Status (v0)

What is wired up today in the repo:

- Zone selection at boot
  - Native and Web builds accept a zone slug via `ROA_ZONE` or URL query `?zone=<slug>`.
  - The platform applies the selection before first frame; the renderer contains no demo/special‑mode branches.

- Snapshot loader (tolerant)
  - `data_runtime::zone_snapshot` loads baked files from `packs/zones/<slug>/snapshot.v1/`.
  - Reads `meta.json` and optional blobs (e.g., `colliders.bin`, `colliders_index.bin`). Missing files are tolerated so formats can evolve safely.

- Client presentation
  - `client_core::zone_client::ZonePresentation::load(slug)` loads a zone snapshot root for the client.
  - The renderer exposes `set_zone_batches(...)` and checks `has_zone_batches()` to decide whether to draw legacy, hardcoded static content. When a Zone is present, the renderer draws Zone static + replicated actors only.

- Bake (tooling)
  - `tools/zone-bake` includes a minimal library API (used by tests) that emits a small `snapshot.v1` (terrain/trees/meta/colliders) deterministically. This is a placeholder for the full baker.

- Tests
  - Schema: serde‑level validation for a minimal `scene.json` structure (deny unknown fields).
  - Client: zone loader reads a temporary snapshot root.
  - Renderer policy: GPU‑free unit tests verify selection/draw policy.

## 13) Next Steps

Data & Schemas
- Define the first `.roazone` (or `scene.json`) authoring schema with JSON Schema + serde. Include instances, logic (spawns/triggers), waypoints, and links.
- Add a `schemas/` directory and validators (via `xtask`) for example scenes under `data/zones/**`.

Bake Pipeline
- Expand `tools/zone-bake` to output full `snapshot.v1/*`:
  - `instances.bin` (static mesh instances), `clusters.bin` (culling grid), `colliders.bin` (+ index), optional `navmesh.bin`, and `logic.bin`.
  - Determinism: enable a fast determinism test in CI (reduced sizes) and keep a heavier one `#[ignore]` for local checks.

Server Integration
- Introduce `server_core::zones` with `ZoneRegistry` and `boot_with_zone(world, slug, specs)` that mounts instances/colliders/navmesh and applies zone logic to spawn initial actors.
  - Example: a `builder_sandbox`/`cc_demo` zone spawns only the PC; a combat zone spawns encounter NPCs.
  - Maintain server authority; the client only renders what is replicated.

Client & Renderer
- Implement CPU decode of baked instance/cluster formats in `client_core::zone_client` and upload to GPU via a new `gfx/zone_batches` path.
- Remove remaining legacy static draws once Zones provide equivalent content in all scenes.
- Keep the renderer free of gameplay nouns; treat Zones + replication as the single source of visual truth.

Site & Routing
- Ensure website/demo routes append `?zone=<slug>` (or set `ROA_ZONE`) so live WASM builds boot the intended Zone without code changes.

Tests & CI
- Golden checks for snapshot meta (counts/bounds) for sample zones to catch drift.
- Grep guards in CI to forbid reintroducing demo branches into the renderer (e.g., `cc_demo`, `demo_mode`).

Assets & Safety
- If large GLBs live in‑repo, use Git LFS. Avoid leaking absolute local paths in logs on WASM.

Docs
- Expand this document with the finalized `.roazone` schema and update any cross‑links.

* [ ] Add **Tile Brush** UI (palette, paint/erase, rotate).
* [ ] Add Outliner/Layers; Save/Load `scene.json`; Bake button (spawn process).
* [ ] Feature `editor_preview` to upload CPU scene to GPU for WYSIWYG.

**Builder Zone (in‑app)**

* [ ] Add `builder_sandbox` zone resources.
* [ ] Builder HUD overlay (same brushes as desktop, minimal UI).
* [ ] Save As New Zone → writes to `data/zones/<slug>`; **optional** invoke baker; hot‑reload.
* [ ] Platform boot param: `--builder` or `?builder=1` to enable overlay in that zone.

**Runtime & Renderer**

* [ ] **Remove** `cc_demo`/demo branches; add `Renderer::set_zone_batches`.
* [ ] Idle/jog/run animation mapping to prevent T‑pose when speed≈0.

**Docs & CI**

* [ ] Update `docs/systems/zone_builder.md` with *Runtime Integration & Builder*.
* [ ] Add schema samples: `tests/fixtures/zones/{builder_sandbox,forest_grove}/scene.json`.
* [ ] Grep guards: forbid `demo`/`cc_demo` in renderer.

---

## 12) Minimal APIs (sketch)

**Editor commands (shared):**

```rust
enum BuilderCmd {
  SetGrid { cell_size: f32, w: u16, h: u16 },
  PaintTile { x: i32, y: i32, asset_id: String, rot: u16, layer: String },
  EraseTile { x: i32, y: i32 },
  PlaceInstance { asset_id: String, transform: Affine3A, layer: String, tags: Vec<String> },
  SetLayerVisible { layer: String, visible: bool },
  SaveScene { slug: String },
  Bake { slug: String },
  Undo, Redo
}
```

**Client Builder HUD → data_runtime:**

```rust
fn apply_cmd(scene: &mut ZoneScene, cmd: BuilderCmd) { /* pure, testable */ }
fn to_instances(scene: &ZoneScene) -> Vec<Instance>; // expands grid + direct instances
```

---

## 13) Testing Strategy (what “done” looks like)

* **Schema tests:** valid/invalid scenes; round‑trip serde.
* **Bake determinism:** two clean temp dirs → identical BLAKE3 hash of artifacts.
* **Tile expansion tests:**

  * Paint A→erase→paint B yields expected `instances` (order & transforms).
  * Rotation set {0,90,180,270} maps to quat Y rotations exactly.
* **Builder commands are pure:** `apply_cmd` unit tests cover all branches; undo/redo linearizability.
* **Runtime smoke:**

  * Boot `builder_sandbox` → 1 PC, 0 NPC, 0 props if empty scene.
  * After painting 10 trees + bake, boot selected zone → static draw count ≥ 10, no wizards.
* **Renderer policy:** draw static when zone batches present; idle anim when Δspeed≈0.

> See prior “test suite for zone loading” for boilerplate structures; extend there.

---

## 14) Risks & Mitigations

* **Scope creep** in in‑app builder → Keep v0.1 minimal (Tile Brush only), escalate to desktop editor for heavy workflows.
* **Asset bloat** → start with a tiny curated set; enforce git‑lfs and catalog gating.
* **Non‑determinism** (scatter later) → centralize RNG seeding; snapshot golden hashes in tests.
* **Security for DM mode** (future) → ACL’d server commands; no custom scripting in live sessions (learn from Neverwinter Foundry constraints).

---

## 15) Milestones & Acceptance

**M0 (1–2 wks):**

* Schema `grid`, Tile Brush (in desktop editor), bake expansion, boot zone by slug, renderer hookup (no demo branches).
  **Accept:** can paint 10 trees, save, bake, play the new zone.

**M1 (1–2 wks):**

* In‑app **Builder Zone** HUD parity (paint/erase/rotate/save/bake).
* Layers, Outliner, basic undo/redo.
  **Accept:** same authoring flow entirely inside the game entrypoint.

**M2 (2 wks):**

* Free‑transform instances + deterministic scatter brush; simple triggers/spawns; navmesh stub.
  **Accept:** zone with spawns loads; NPCs appear only when logic says so.

**Runway (v1.x):**

* DM Control Room; height‑map terrain; share/publish flow (catalog, later Workshop‑like).

---

## 16) Why this will feel great to creators

* **Fast path to “I made a place.”** Tile painting + tiny palette = immediate success (proven in NWN/Grimrock).
* **One button to bake & play.** Tight loop keeps iteration fun.
* **No tech debt in renderer.** Zones decide content; renderer stays clean.
* **Clear path to DM/live sessions.** The Builder Zone is the same canvas the DM will use later (in spirit to NWN DM / DOS2 GM).

---

### Appendix A — References we’re consciously echoing

* **NWN Aurora Toolset:** approachable tile editor + deep scripting + DM client.
* **Grimrock Editor:** grid paint, quick iteration, community mods/workshop.
* **Solasta Dungeon Maker:** drag‑drop rooms, “gadgets” (triggers), sharing pipeline.
* **Divinity OS2 GM Mode:** live orchestration, possess NPCs, improvise in session.

---

**Next actions (concrete):**

1. Add `grid` to schema + tests.
2. Implement Tile→Instance expansion in baker.
3. Build Tile Brush in `tools/zone-editor`.
4. Add `builder_sandbox` zone + in‑app Builder HUD with Save/Bake.
5. Remove renderer demo branches; wire `Renderer::set_zone_batches`.
6. Land tests and grep guards.

That’s the definitive spec for v0.1 with a clean runway to DM and terrain.
