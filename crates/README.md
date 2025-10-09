# Workspace Crates Overview

This document summarizes the crates in `crates/` and what each is responsible for. It’s a quick map for contributors; see per‑crate rustdoc and source for details.

Note: Shared libraries that aren’t under `crates/` (e.g., `shared/assets` as `roa-assets`) are not listed here.

## Crates

### render_wgpu
- WGPU renderer. Hosts the full renderer under `src/gfx/**` (camera, pipelines, shaders, UI overlays, scene assembly, terrain, sky, temporal, helpers).
- Exposes `gfx::Renderer` used by the app/platform. Integrates with `client_core` (input/controller facade), `ecs_core` (components), `ux_hud`, `data_runtime` (zone/TOD), and replication from `net_core`.
- Optional bits: `server_ext` (renderer-side helper traits when a server is present). Demo bin: `--bin vox_onepath` behind `vox_onepath_demo` feature.

### platform_winit
- Platform/window/input loop built on winit 0.30. Provides an `ApplicationHandler` that creates a window/canvas and drives `render_wgpu::gfx::Renderer`.
- Wires simple in‑proc replication via `net_core::transport::LocalLoopbackTransport`.
- Feature `demo_server` (default) spawns an in‑proc `server_core::ServerState` for the demo.

### data_runtime
- SRD‑aligned data models and loaders. Replaces the old `src/core/data` facade.
- Modules: `specdb` (content facade), `spell`, `class`, `ability`, `monster`, `scenario`, `scene` (destructibles), `zone` (authoring manifest with TOD/terrain/weather), and `configs/*` (destructible, input/camera, telemetry, PC animations, NPC unique).
- Serializes/deserializes JSON/YAML/TOML where helpful; designed for deterministic authoring flows.

### ecs_core
- Minimal ECS scaffolding and shared components/types for server/client integration.
- Components include destructible metadata, voxel proxies and dirty/mesh queues, carve requests, chunk meshes, simple controller/camera facades, actor/boss tags, defenses/statuses, and collision shapes.
- `parse` provides string→enum helpers for data‑driven configs. Optional `replication` feature gates serde derives on some types.

### client_core
- Client glue: input/controller state, a simple third‑person controller, camera integration helpers, and upload/replication scaffolding.
- `systems/*` update controller and camera; `replication` buffers/apply deltas; `upload` defines a mesh‑upload interface consumed by the renderer.
- Exposes a read‑only controller facade for the renderer.

### client_runtime
- Thin client‑side runtime to decouple controller + collision updates from the renderer.
- Produces `SceneInputs` the renderer can consume to update player transform and camera without owning input/collision policy.

### net_core
- Snapshot schema + encode/decode traits, frame format, interest management, client→server command messages, and an in‑proc transport.
- Modules: `frame` (RAF1 framing), `snapshot` (encode/decode + messages like chunk mesh deltas, hit fx, HUD toasts, destructible AABBs), `command` (authoritative input intents), `interest` (spherical interest helpers), `channel` (bounded loopback), `transport` (trait + loopback impl), `apply` (client‑side apply scaffold).

### server_core
- Authoritative server state and systems: NPC AI/perception/movement, projectile integration and collision, destructible tick (carve→mesh→collider with budgets), replication, and interest.
- Uses `voxel_proxy` + `voxel_mesh` for destructibles and `collision_static` for static colliders; shares components from `ecs_core`.
- Demo scaffolding: ring spawns for undead, a unique boss (e.g., Nivita), wizard NPC casters, simple anti‑overlap, and a local replication channel. Optional Prometheus exporter on native.

### sim_core
- Rules + combat scaffolding and a deterministic headless simulation runtime.
- Hosts `rules/*` (SRD helpers), `combat/*` (FSM, damage/conditions), and `sim/*` (fixed‑tick scheduler and systems). Rendering is out of scope.

### ux_hud
- HUD logic/state with simple toggles. Produces lightweight, flattened draw data for a renderer UI module to consume.

### collision_static
- Coarse static colliders for voxel chunks (chunk OBBs and world AABBs) and simple capsule/cylinder‑vs‑static slide resolution.
- Builds per‑chunk colliders, flattens into a static index for queries; favors robustness for demos.

### core_units
- Lightweight unit types and helpers (e.g., `Length`, `Time`, `Mass`) with typed arithmetic and conversions.
- Shared by material/voxel/collision code for clarity and determinism.

### core_materials
- Static material palette with densities and display albedos; name→ID lookup; helpers to compute mass from voxel size and density.
- Used by destructibles/debris math and related systems.

### voxel_proxy
- Chunked voxel grid representation and operations for destructibles.
- Voxelization helpers (surface mark + flood‑fill), carve operations that track dirty chunks and removed voxel centers, and proxy metadata tying grids to design objects and materials/units.

### voxel_mesh
- CPU‑only greedy meshing over `voxel_proxy::VoxelGrid`.
- Generates triangle buffers from solid→empty boundaries; per‑chunk meshing helpers for dirty sets.

---

Conventions
- Keep crates dependency‑light and focused. Renderer/platform/web APIs should not leak into gameplay/sim/data crates.
- Prefer adding unit tests alongside new functionality (math/transforms, parsing, voxel ops, replication encode/decode, etc.).
- If you add a new workspace crate, update this file with a brief scope and primary consumers.

