# Workspace Crates Overview

This document summarizes all crates under `crates/` and their responsibilities. It’s a quick map for contributors; see per‑crate rustdoc and source for details. Shared libraries outside `crates/` (e.g., `shared/assets` as `roa-assets`) and application crates (e.g., `apps/roa_slice_bevy`) are called out where relevant.

## Crates

### roa_domain (portable gameplay domain)
- Portable ECS domain used by the Bevy slice and (later) the runtime. No Bevy meta‑crate dependency.
- Modules: `input` (Message commands), `character` (DragonController + TransformState), `sim_time` (FixedUpdate time), `npc` (Npc/NpcKind + SpawnNpc Message).
- Consumers: `apps/roa_slice_bevy` (input→domain wiring; NPC spawn/visual adapter).

### render_wgpu
- WGPU renderer. Full renderer lives under `src/gfx/**` (camera, pipelines, shaders, UI overlays, scene assembly, terrain, sky, temporal, helpers).
- Exposes `gfx::Renderer` used by the platform. Integrates with `client_core`, `ecs_core`, `ux_hud`, `data_runtime`, and `net_core` replication.
- Zone integration: `gfx::zone_batches` API for static batches. Demo bin: `--bin vox_onepath` behind `vox_onepath_demo` feature.

### platform_winit
- Platform/window/input loop (winit 0.30). Implements an `ApplicationHandler` that creates a window/canvas and drives `render_wgpu::gfx::Renderer`.
- Zone picker + zone load, optional in‑proc demo server feature (`demo_server`). Wires loopback replication (`net_core`).

### data_runtime
- SRD‑aligned data models and loaders. Replaces old `src/core/data` facade.
- Modules: `specdb`, `spell`, `class`, `ability`, `monster`, `scenario`, `scene` (destructibles), `zone` (authoring manifest with TOD/terrain/weather), `zone_scene` (scene schema), `zone_snapshot` (snapshot loader + registry), `configs/*` (input/camera, telemetry, PC anims, NPC unique).

### ecs_core
- Minimal ECS scaffolding and shared components/types for server/client integration.
- Components include destructible metadata, voxel proxies and dirty/mesh queues, carve requests, chunk meshes, simple controller/camera facades, actor/boss tags, defenses/statuses, collision shapes.
- Optional `replication` feature gates serde derives.

### client_core
- Client glue: input/controller state, simple third‑person controller, camera helpers, and upload/replication scaffolding.
- `zone_client::ZonePresentation::load(slug)` prepares snapshot roots; platform uploads batches to the renderer.

### client_runtime
- Thin client‑side runtime to decouple controller + collision updates from the renderer.
- Produces `SceneInputs` the renderer can consume.

### net_core
- Snapshot schema + encode/decode traits, frame format, interest management, client→server command messages, and loopback transport.
- Modules: `frame` (RAF1), `snapshot` (encode/decode + messages), `command` (authoritative intents), `interest`, `channel`, `transport`, `apply`.

### server_core
- Authoritative server state and systems: NPC AI/perception/movement, projectile collision, destructible tick, replication/interest.
- Uses `voxel_proxy` + `voxel_mesh` and `collision_static`; shares components with `ecs_core`.

### sim_core
- Rules + combat scaffolding and deterministic headless simulation runtime.
- Hosts `rules/*` (SRD helpers), `combat/*` (FSM, damage/conditions), `sim/*` (fixed‑tick scheduler/systems).

### ux_hud
- HUD logic/state with simple toggles. Produces flattened draw data for a renderer UI module.

### collision_static
- Static colliders for voxel chunks (chunk OBBs and world AABBs) and simple capsule/cylinder‑vs‑static slide resolution.

### core_units
- Strongly‑typed units (`Length`, `Time`, `Mass`) with conversions. Used by voxel/collision/material code.

### core_materials
- Static material palette with densities and display albedos; helpers to compute mass from voxel size and density.

### voxel_proxy
- Chunked voxel grid representation and operations for destructibles. Voxelization helpers; carve ops that track dirty chunks and removed voxel centers; proxy metadata tying grids to design objects and materials/units.

### voxel_mesh
- CPU‑only greedy meshing over `voxel_proxy::VoxelGrid`. Generates triangle buffers from solid→empty boundaries; meshing helpers for dirty sets.

### worldsmithing
- In‑world authoring logic for V1 “Place Tree”. Pure, UI/renderer‑agnostic crate that owns placement state, yaw rotation, caps/rate‑limit, and export/import (serde) of authoring files.
- Intended usage: platform routes inputs to this crate; renderer provides an ephemeral ghost draw; tools/zone‑bake consumes exported `scene.json` to produce grouped `trees.json` for instancing.
- Keeps authoring decisions out of the renderer and platform; data‑driven via a small catalog (global with per‑zone overrides in `data/`).
### dev_docs
- Rustdoc aggregator for developer documentation. Builds a browsable docs site from Markdown using `cargo doc -p dev_docs --no-deps`.
- Short‑term convenience until we stand up mdBook or a site; no runtime dependencies.

---

Conventions
- Keep crates dependency‑light and focused. Renderer/platform/web APIs should not leak into gameplay/sim/data crates.
- Prefer adding unit tests alongside new functionality (math/transforms, parsing, voxel ops, replication encode/decode, etc.).
- If you add a new workspace crate, update this file with a brief scope and primary consumers.

### wishcraft
- Core Wish system: schema models (serde), linting, scoring (Clarity/Safety/Reversibility), Heat estimation, Genie Registry traits, Ledger entry models, and Shadow‑Run/Execute traits.
- Feature flags:
  - `sim`: adapters to run shadow simulations against `sim_core` snapshots (traits only in skeleton).
  - `server`: helpers for transactional apply on the server (traits only in skeleton).
  - `schemars`: emit JSON Schema for authoring tools.
  - `fs-ledger`: file‑backed ledger helpers (reserved).
- Consumers: `ux_hud` (UI flow), `server_core` (apply + ledger), `xtask` (lint/shadow‑run/court CLI).

### wishcraft_openai
- Optional OpenAI bridge for Wishcraft flows (prompt construction, tool schemas). Kept isolated to avoid runtime coupling; used by `xtask`/tools or experiment bins only.
