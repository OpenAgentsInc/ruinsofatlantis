Awesome—here’s a complete **copy‑pasteable** set of GitHub issues that break your Epic into small, agent‑friendly chunks. Each issue includes labels, dependencies, concrete file paths, tasks, and acceptance criteria.

> **Note:** Replace references to **#95** with your actual epic issue number after you create it. These are written to be opened as **separate issues** in the order shown.

---

## 1) Phase 0 – Preflight hygiene & feature gates

**Title:** Phase 0 — Preflight hygiene: feature‑gate legacy paths, tighten logging

**Labels:** `refactor` `cleanup` `tech-debt`
**Depends on:** Epic #95

### Summary

Before we cut systems over to the server, gate any legacy/demo paths so changes don’t leak into the main runtime and reduce default log noise.

### Scope (repo‑aware)

* `crates/render_wgpu/src/gfx/renderer/update.rs` — demo voxel helpers (`vox_onepath`), client‑side carve/mutate code paths.
* Logging across `render_wgpu`, `server_core::destructible`.

### Tasks

* [ ] Add Cargo feature flags:

  * [ ] `legacy_client_carve` (default **off**)
  * [ ] `vox_onepath_demo` (default **off**)
  * [ ] `destruct_debug` (opt‑in chatter)
* [ ] Wrap demo grid, one‑path viewer, and any client‑side carve/collider/mesh mutations with `#[cfg(feature = "legacy_client_carve")]` or `vox_onepath_demo`.
* [ ] Move verbose `info!`/`warn!` logs under `destruct_debug`.
* [ ] Remove/disable `RA_VOX_DEMO` env behavior; prefer feature flags only.

### Acceptance

* [ ] Default build shows **no** demo grid and **no** client‑side carve/mutate.
* [ ] Enabling `legacy_client_carve` restores prior behavior for side‑by‑side testing.
* [ ] CI passes.

---

## 2) Scaffolds – client_core & net_core crates

**Title:** Add scaffolds for `client_core` and `net_core` crates

**Labels:** `infrastructure` `networking`
**Depends on:** Epic #95

### Summary

Create empty crates to host replication and client‑side systems, with CI wired.

### Tasks

* [ ] Add `crates/client_core` and `crates/net_core` with `lib.rs`.
* [ ] Hook into workspace `Cargo.toml`, `xtask ci`.
* [ ] Add minimal module stubs:

  * `client_core/src/{replication.rs,upload.rs,systems/mod.rs}`
  * `net_core/src/{snapshot.rs,apply.rs,interest.rs}`

### Acceptance

* [ ] Workspace builds with new crates.
* [ ] CI executes clippy/tests in both crates.

---

## 3) ECS components for destructibles & chunk mesh

**Title:** ECS Components — Destructible, VoxelProxy, ChunkDirty, ChunkMesh, CarveRequest

**Labels:** `ecs` `server-authoritative` `voxel`
**Depends on:** #95, Scaffolds

### Files to touch

* `crates/ecs_core/src/components.rs`
* `crates/ecs_core/src/lib.rs` (exports)

### Tasks

* [ ] Add components (derive Debug/Clone where useful):

  * [ ] `Destructible { id: u64, material: MaterialId }`
  * [ ] `VoxelProxy { meta: voxel_proxy::VoxelProxyMeta /* server holds grid handle out-of-band */ }`
  * [ ] `ChunkDirty(Vec<glam::UVec3>)`
  * [ ] `ChunkMesh { map: HashMap<(u32,u32,u32), MeshCpu> }` (define simple `MeshCpu { positions: Vec<[f32;3]>, normals: Vec<[f32;3]>, indices: Vec<u32> }`)
  * [ ] `CarveRequest { did: u64, center_m: glam::DVec3, radius_m: f64, seed: u64, impact_id: u32 }`
* [ ] Add `EntityId(u64)` and `DestructibleId(u64)` newtypes (serde if needed).
* [ ] Rustdoc each component with who mutates it (server vs client).

### Acceptance

* [ ] Components compile, unit test constructs an entity set including a `ChunkMesh` map entry.

---

## 4) Data runtime for destructible & budgets

**Title:** Data config — destructible budgets & tuning in `data_runtime`

**Labels:** `data` `config` `voxel`
**Depends on:** #95

### Files

* `crates/data_runtime/src/configs/destructible.rs` (new)
* `crates/data_runtime/src/lib.rs` (export)
* `crates/server_core/src/destructible/config.rs` (wire CLI overrides)

### Tasks

* [ ] Define `DestructibleConfig`: `voxel_size_m`, `chunk: UVec3`, `aabb_pad_m`, `max_remesh_per_tick`, `collider_budget_per_tick`, `max_debris`, `seed`, `close_surfaces`, `max_carve_chunks`.
* [ ] Loader from TOML/JSON (choose one consistent with repo).
* [ ] Map CLI flags to override fields.
* [ ] Unit tests: round‑trip load + overrides.

### Acceptance

* [ ] Server boot reads defaults; CLI overrides reflect in effective config (log one line summary).

---

## 5) Server systems — Voxel carve, mesh, collider (authoritative)

**Title:** Server systems — VoxelCarve, GreedyMesh, ColliderRebuild (budgeted)

**Labels:** `server-authoritative` `ecs` `jobs` `voxel`
**Depends on:** Components, Data config

### Files

* `crates/server_core/src/systems/destructible.rs` (new module)
* `crates/server_core/src/systems/mod.rs`
* `crates/server_core/src/tick.rs` (system ordering)
* `crates/server_core/src/collision_static/chunks.rs` (ported builder)
* `crates/server_core/src/jobs/{mod.rs,thread_pool.rs}` (see Issue 10 below if split)

### Tasks

* [ ] `VoxelCarveSystem`: consume `CarveRequest`, call `carve_and_spawn_debris`, mark `ChunkDirty`.
* [ ] `GreedyMeshSystem`: pop `ChunkDirty` with **budget**, run `voxel_mesh::greedy_mesh_chunk`, write `ChunkMesh.map` entries.
* [ ] `ColliderRebuildSystem`: rebuild per‑chunk colliders (ported from `render_wgpu::gfx::chunkcol`), update a server‑side spatial index.
* [ ] Add timing metrics per system; no work on render thread.
* [ ] Unit tests for:

  * [ ] Deterministic carve count for fixed seed.
  * [ ] Mesh quads > 0 after carve.
  * [ ] Collider count == dirty chunk count.

### Acceptance

* [ ] With a seeded grid + one `CarveRequest`, server produces `ChunkMesh` entries and colliders within budgeted ticks.
* [ ] Runs in fixed tick without panics; timing logged.

---

## 6) Renderer: stop mutating world; add upload helper

**Title:** Renderer cleanup — remove carve/collider mutation; add `voxel_upload` module

**Labels:** `renderer` `refactor`
**Depends on:** Phase 0 gates

### Files

* `crates/render_wgpu/src/gfx/renderer/update.rs`
* `crates/render_wgpu/src/gfx/renderer/voxel_upload.rs` (new)
* `crates/render_wgpu/src/gfx/renderer/init.rs` (minor)
* `crates/render_wgpu/src/gfx/renderer/render.rs` (unchanged)

### Tasks

* [ ] Guard or delete client‑side carve/collider/mesh mutation code behind `legacy_client_carve`.
* [ ] Extract VB/IB creation for a single chunk into `voxel_upload::upload_chunk_mesh(did, chunk, MeshCpu)`.
* [ ] Replace local maps (`voxel_meshes`, `voxel_hashes`) with versions keyed by **typed** `DestructibleId`.
* [ ] Add CPU‑only unit test feeding a tiny `MeshCpu` to verify indexing/normals.

### Acceptance

* [ ] In default build, renderer **only** uploads provided meshes; no carves happen client‑side.
* [ ] Visual output unchanged when server streams meshes.

---

## 7) Server projectile → damage pipeline

**Title:** Server systems — Projectiles, Collision, Damage

**Labels:** `ecs` `server-authoritative` `combat`
**Depends on:** Data/SpecDb (Issue 8), Components, Tick scheduler

### Files

* `crates/server_core/src/systems/{projectiles.rs,collision.rs,damage.rs}`
* `crates/server_core/src/tick.rs` (insert ordering)

### Tasks

* [ ] `ProjectileIntegrateSystem` (fixed dt): integrate `Projectile`, produce segment per tick.
* [ ] `CollisionSystem`: broadphase grid; test segments vs `CollisionShape` and destructible AABBs; emit hit events.
* [ ] `DamageApplySystem`: mutate `Health`, emit death events; forward destructible hits to `CarveRequest`.
* [ ] Unit tests for deterministic segment creation, hit pairs.

### Acceptance

* [ ] With canned entities and specs, server tick produces consistent hits and damage; destructible hits create `CarveRequest`s.

---

## 8) Data runtime — Projectile SpecDb

**Title:** SpecDb — Projectile specs (speed, radius, damage, lifetime)

**Labels:** `data` `combat`
**Depends on:** Epic #95

### Files

* `crates/data_runtime/src/specs/projectiles.rs` (new)
* `crates/data_runtime/src/specdb.rs` (wire)
* Data files in `/data/projectiles/*.toml|json`

### Tasks

* [ ] Define schema + loader + validation.
* [ ] Replace hard‑coded constants in server projectile systems.
* [ ] Expose read‑only view for client prediction.

### Acceptance

* [ ] Unit tests pass; server logs spec id on projectile spawn; no constants left in code.

---

## 9) Replication v0 — snapshots, deltas, interest (local loop)

**Title:** Replication v0 — net_core snapshots/deltas + interest grid

**Labels:** `networking` `replication`
**Depends on:** Components, Server systems Phase 1–2, client_core scaffold

### Files

* `crates/net_core/src/snapshot.rs`
* `crates/net_core/src/interest.rs`
* `crates/client_core/src/replication.rs`
* `crates/net_core/src/apply.rs`

### Tasks

* [ ] Define snapshot records:

  * [ ] `EntityHeader { id, archetype }`
  * [ ] Components: `Transform`, `Health`, `Destructible`, `VoxelProxyMeta`, `ChunkMeshDelta { did, chunk, positions, normals, indices }`, `Projectile`
* [ ] Implement simple baselining/delta for `ChunkMeshDelta` (full replace OK in v0).
* [ ] Interest grid: include only entities/chunks in N cells around player.
* [ ] Client: apply snapshots → create/update components; invalidate uploads when mesh deltas arrive.

### Acceptance

* [ ] With a single‑process loop, meshes from server appear on client; moving away stops updates (interest filter).

---

## 10) Lightweight job scheduler (server)

**Title:** Job scheduler — budgeted mesh/collider/voxelize jobs

**Labels:** `jobs` `performance`
**Depends on:** Server systems Phase 1

### Files

* `crates/server_core/src/jobs/{mod.rs,thread_pool.rs,job_types.rs}`

### Tasks

* [ ] Thread pool with work queues.
* [ ] Job types: `MeshChunkJob`, `BuildColliderJob`, `VoxelizeSurfaceJob`.
* [ ] Budgets per tick; expose metrics (jobs dispatched/completed, ms).
* [ ] Integrate with GreedyMesh/Collider systems.

### Acceptance

* [ ] Server stays responsive when flooding `ChunkDirty`; budgets honored; metrics log each tick.

---

## 11) Client upload pipeline

**Title:** client_core → render_wgpu upload bridge for `ChunkMesh`

**Labels:** `client` `renderer`
**Depends on:** Replication v0, Renderer upload module

### Files

* `crates/client_core/src/upload.rs`
* `crates/render_wgpu/src/gfx/renderer/voxel_upload.rs`

### Tasks

* [ ] Implement a channel or callback from `client_core::upload` to renderer’s `voxel_upload`.
* [ ] Ensure dedupe by `(DestructibleId, chunk)`; handle deletes (empty mesh removes GPU buffers).

### Acceptance

* [ ] Mesh updates applied on client cause visible voxel surfaces to appear/update; stale chunks are removed.

---

## 12) Scene build (server) — data‑driven destructibles

**Title:** Scene build — data‑driven destructible registry on server

**Labels:** `scene` `data` `server-authoritative`
**Depends on:** Components, Data Runtime

### Files

* `crates/server_core/src/scene_build.rs` (new)
* `crates/data_runtime/src/schemas/scene_destructibles.json` (or TOML)
* Remove client‑side GLTF load in `crates/render_wgpu/src/gfx/scene.rs` (consume replication instead)

### Tasks

* [ ] Parse GLTF extras or external scene data that marks nodes as destructible.
* [ ] Compute local AABB per mesh, world AABB per instance; spawn ECS entities:

  * `Destructible`, initial `VoxelProxy` (inactive until hit), optional `Renderable`.
* [ ] Replicate `Destructible` + `VoxelProxyMeta` to client.
* [ ] On first hit: replicate “hide source renderable” event (generic, not ruins‑specific).

### Acceptance

* [ ] Any tagged model (not just ruins) becomes destructible; renderer no longer loads GLTF for destructibles.

---

## 13) Renderer cleanup & de‑ruin

**Title:** Renderer cleanup — remove ruins‑specific destructible glue

**Labels:** `renderer` `cleanup`
**Depends on:** Scene build on server

### Tasks

* [ ] Delete or guard `get_or_spawn_ruin_proxy`, `hide_ruins_instance`, ruins‑specific selection.
* [ ] Ensure keys for voxel buffers are `(DestructibleId, cx, cy, cz)` everywhere.
* [ ] Keep a dev overlay for per‑proxy stats (optional).

### Acceptance

* [ ] No ruins‑specific logic under default build; destructibles are model‑agnostic.

---

## 14) NPCs into ECS (server‑side)

**Title:** NPC ECS — components + AI/perception/action systems

**Labels:** `ecs` `ai` `server-authoritative`
**Depends on:** Projectile/Damage systems, Replication v0

### Files

* `crates/server_core/src/systems/npc.rs` (new)
* Remove/port logic from `crates/server_core/src/lib.rs` (current `ServerState` vectors)

### Tasks

* [ ] Components: `Npc { radius, speed }`, `Transform`, `Velocity`, `Health`, `Team`.
* [ ] Systems: `NpcPerceptionSystem`, `NpcAiSystem`, `NpcResolveCollisionsSystem`, `NpcMeleeSystem`, `NpcDeathSystem`.
* [ ] Replicate `Transform`/`Health` to client.
* [ ] Renderer consumes replication only (no direct `server.npcs` access).

### Acceptance

* [ ] NPCs move/attack server‑side; client visuals follow replicated transforms; damage floaters use replicated events.

---

## 15) Player controller & camera to client_core

**Title:** Client controller & camera — move from renderer to client_core

**Labels:** `client` `controls`
**Depends on:** client_core scaffold

### Files

* `crates/client_core/src/systems/controller.rs` (new)
* `crates/render_wgpu/src/gfx/renderer/update.rs` (remove `apply_pc_transform` logic)

### Tasks

* [ ] Move player movement integration + terrain clamp to `client_core`.
* [ ] Provide API to renderer to retrieve PC transform for GPU upload.
* [ ] Config (speeds/yaw) from `data_runtime`.

### Acceptance

* [ ] No transform math in renderer; camera and PC motion updated by client_core; visuals unchanged.

---

## 16) Tests & CI expansion

**Title:** Tests & CI — system unit tests, integration, WGSL validation, advisories

**Labels:** `ci` `testing`
**Depends on:** Phases 1–3

### Tasks

* [ ] Unit tests:

  * [ ] Projectile integration determinism.
  * [ ] Collision candidate counts.
  * [ ] Carve voxel counts (tolerance).
  * [ ] Mesh quad counts after carve.
* [ ] Integration tests (server tick): N ticks → assert entity counts, health deltas, dirty chunk sizes.
* [ ] Replication tests: encode/decode snapshots; interest filters.
* [ ] CI:

  * [ ] Naga WGSL validation step (existing shaders).
  * [ ] `cargo deny` advisories.
  * [ ] Perf smoke: enforce budgets (mesh/collider jobs per tick).

### Acceptance

* [ ] CI green with new suites; failures point to deterministic deltas.

---

## 17) Remove legacy client carve (post‑migration)

**Title:** Remove `legacy_client_carve` & demo codepaths (cleanup)

**Labels:** `cleanup`
**Depends on:** Phases 1–3 complete; Renderer upload path proven

### Tasks

* [ ] Delete gated carve/collider code.
* [ ] Delete demo grid/one‑path viewer unless explicitly kept under `vox_onepath_demo` for tooling only.
* [ ] Update docs.

### Acceptance

* [ ] No remaining paths mutate voxels/colliders on client; repo simpler.

---

## 18) Docs & ADR

**Title:** ADR — Server‑authoritative ECS & job system; Contributing updates

**Labels:** `docs` `adr`
**Depends on:** Epic #95

### Tasks

* [ ] `docs/adr/000X-ecs-server-authority.md` capturing decisions & tradeoffs.
* [ ] Update `CONTRIBUTING.md` with system phases, ticks, budgets, feature flags.
* [ ] High‑level architecture diagram (ASCII ok).

### Acceptance

* [ ] ADR merged; contributors have a clear checklist to add new systems.

---

## 19) Optional: Metrics overlay & dev toggles

**Title:** Dev overlay — per‑system timings, budgets, and backlogs

**Labels:** `developer-experience` `observability`
**Depends on:** Job scheduler, Server systems

### Tasks

* [ ] Server: expose counters for mesh/collider jobs, carve counts, timings.
* [ ] Client: overlay panel reads replicated metrics or in‑proc hooks during local run.

### Acceptance

* [ ] Hotkey toggles overlay; shows red when budgets exceeded.

---

## 20) Data‑driven scene tagging & sample content

**Title:** Authoring — data schema & sample tagging for destructibles

**Labels:** `authoring` `data`
**Depends on:** Scene build

### Tasks

* [ ] Schema for destructible tags per GLTF node or external scene JSON.
* [ ] Convert current ruins to data‑driven tags.
* [ ] Document tagging in README of `data_runtime`.

### Acceptance

* [ ] New scenes can mark arbitrary meshes as destructible without code changes.

---

### Coordination / Boards

* Create a project board with columns: **Backlog → In Progress → Review → Done**.
* Add these issues in order and set dependencies (`Blocked by`) to guide agents.
* For each issue, cut PRs with small scopes (≤ 500 LOC net change ideally), include tests and **update the epic checklist**.

If you want, I can also generate **PR templates/checklists** for each issue so agents can open compliant PRs with minimal back‑and‑forth.
