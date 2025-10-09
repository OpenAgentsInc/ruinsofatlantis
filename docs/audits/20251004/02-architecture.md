# Architecture & Layering — 2025-10-04

Inventory
- Crates observed (via cargo tree): `client_core`, `client_runtime`, `collision_static`, `core_materials`, `core_units`, `data_runtime`, `ecs_core`, `net_core`, `platform_winit`, `render_wgpu`, `server_core`, `sim_core`, `ux_hud`, `voxel_mesh`, `voxel_proxy` (docs/audits/20251004/evidence/cargo-tree.txt).
- Workspace members grep saved: docs/audits/20251004/evidence/workspace-members.txt.

Desired layering (checked)
- shared/* utilities (e.g., `roa-assets`) must not depend on game ECS — OK.
- `data_runtime` ECS‑agnostic — OK; no `use ecs_core` found (evidence/layering-ecs-use.txt).
- `server_core` owns authority; `client_core` owns input/prediction/UI; `render_wgpu` draws only — Needs work.
- `net_core` provides encode/decode/channel — Present and used by client/server.

Violations
- Renderer spawns server boss: `crates/render_wgpu/src/gfx/npcs.rs:116` calls `server.spawn_nivita_unique(...)` (evidence/spawn-unique.txt). Ownership should be in app/server initialization.
- Renderer contains gameplay logic and state mutation: `crates/render_wgpu/src/gfx/renderer/render.rs:12` updates scene inputs, camera/controller, simple AI.

Notes
- `server_core::tick` orchestrates destructible work via a job scheduler and budgets (crates/server_core/src/tick.rs:1).
- Replication buffer used in renderer for local loop uploads (render path) — keep decode/upload client‑side, but authoritativeness must remain server‑side.

Recommendations
- Move server entity creation (boss spawn) out of renderer; call from app/bootstrap using `server_core` API.
- Extract input/controller/AI systems into `client_core`; renderer consumes component data and uploads meshes only.
- Keep `net_core` channel construction and snapshot decode solely in client/server crates; renderer should only react to resulting CPU mesh updates.

