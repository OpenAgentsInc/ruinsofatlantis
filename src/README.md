# Source Layout Overview

This document summarizes the `src/` folder structure and what each module does.

Workspace crates (added for modularization)
- crates/data_runtime — SRD-aligned data schemas + loaders (replaces `src/core/data`; re-exported under `crate::core::data`).
- crates/render_wgpu — Renderer crate. The full contents of the old `src/gfx/**` now live here under `crates/render_wgpu/src/gfx/**`. The root `src/gfx/mod.rs` is a thin re‑export of `render_wgpu::gfx`.
- crates/sim_core — Rules/combat/sim crate (moved from `src/core/{rules,combat}` and `src/sim`). Re-exported under `crate::core::{rules,combat}` and `crate::sim` for compatibility.
- crates/platform_winit — Platform loop crate. Root app calls `platform_winit::run()`.
- crates/ux_hud — HUD logic crate (now owns perf/HUD toggles; P toggles perf overlay, H toggles HUD).

- Workspace crates (new)
- shared/assets — Library crate with asset loaders for tools and renderer.
- tools/model-viewer — Standalone wgpu viewer that loads GLTF/GLB via shared/assets.

- lib.rs — Crate root; re‑exports main modules.
- main.rs — Binary entry; sets up logging and runs the winit platform loop.
- platform_winit.rs — Window/event loop integration using winit 0.30.

## Controls
- ALT: toggle cursor ↔ mouselook
- RMB hold: temporarily capture pointer for camera orbit (mouselook)
- 1/2/3: cast spells (demo bindings)
- LMB/RMB: no at‑will casting (reserved for look/forward‑chord only)
- Scroll: zoom in/out
- WASD: movement
  - A/D swing the camera left/right; the player auto‑faces the camera after a short delay
  - Q/E are dedicated strafes (Q = left, E = right)
  - Basis: with RMB held (or LMB+RMB), movement is camera‑relative; otherwise character‑facing relative
- Shift: run
  - Only when holding W; does not apply while strafing/backpedaling
  - Increases forward speed by ~30% (tunable)
- Space: Jump (when PC is alive); when dead, toggles sky pause
- [: scrub time backward a bit; ]: forward a bit
- - / =: halve / double time scale
- P: toggle perf overlay (frametime, FPS, draw calls)
- H: hide/show HUD
- O: 5s automated orbit for screenshots

Worldsmithing (Campaign Builder zone)
- 1: select Place Tree
- B: toggle builder overlay (help + counts)
- Enter / Left Click: confirm placement
- Q/E or Mouse Wheel: rotate ±15° (Ctrl + Wheel: ±1°)
- X / I: export / import authoring data
- Z: undo last placement (single step)

Notes
- Default is third‑person MMO with orbit and auto‑face.
- Auto‑face (camera → character): normal delay ≈ 0.25 s; while RMB is held, delay ≈ 0.125 s. For large swings (>90°) the player turns immediately, trailing just under 90°, then finishes after the delay.
- Pointer‑lock may be denied by the OS/browser; when denied, we fall back to cursor mode and keep the UI interactive.

Config (optional)
- `data/config/input_camera.toml` (if present) adjusts mouselook and ALT behavior.
  Example:
  
  ```toml
  sensitivity_deg_per_count = 0.12
  invert_y = false
  min_pitch_deg = -75
  max_pitch_deg = 75
  alt_hold = true           # ALT acts as hold (press=cursor, release=mouselook)
  profile = "ActionCombat"  # or "ClassicCursor"
  ```

CLI/Env toggles
- `--no-vsync` (or `RA_NO_VSYNC=1`): prefer Immediate present mode if supported.

## Time‑of‑Day (TOD) authoring
- The initial TOD is controlled per zone in `data/zones/<slug>/manifest.json`:
  - `start_time_frac` (fraction `[0..1]`; 0.5 = noon, ~0.0/1.0 = midnight)
  - `start_paused` (boolean; if true, TOD is paused at startup)
  - `start_time_scale` (float; TOD rate when not paused)
- The sky system reads these values at startup and recomputes sun direction, sky parameters
  and SH ambient. At night we darken sky radiance and ambient to achieve an actually dark look.

- client/
  - mod.rs — Client runtime systems index (input/controllers).
  - input.rs — Input state (WASD + Shift) for the player controller.
  - controller.rs — Third‑person controller: A/D turn in place, W forward, S back.

- assets/
  - mod.rs — Public re‑exports for asset loading modules.
  - types.rs — CPU asset types (CpuMesh, SkinnedMeshCPU, AnimClip, Tracks, TextureCPU).
  - gltf.rs — Unskinned GLTF mesh loader + JSON/Draco fallback.
  - skinning.rs — Skinned mesh loader (JOINTS/WEIGHTS) and animation clip extraction.
  - draco.rs — Native Draco decode helpers for mesh/skinned primitives.
  - util.rs — Path preparation per policy (prefer pre‑decompressed assets).

- tools/ (workspace crates)
  - model-viewer — Standalone wgpu viewer for GLTF/GLB.
  - gltf-decompress — One‑time CLI to decompress Draco GLTFs (offline step).
  - image-probe — Simple image IO experiments.

Note: the old `core/` facade has been removed; crates use `data_runtime` and `sim_core` directly.

- ecs/
  - mod.rs — Minimal ECS scaffolding (entities, transforms, render kinds).

- gfx/
  - mod.rs — Thin re‑export of `render_wgpu::gfx`.
  - renderer/ — Extracted renderer internals split by responsibility:
    - init.rs — Full constructor (`Renderer::new_core`) moved here; `gfx::Renderer::new()` delegates
    - render.rs — Full frame render path moved here; `gfx::Renderer::render()` delegates
    - passes.rs — Post/overlay passes invoked from render()
    - resize.rs — Swapchain + attachments rebuild on window resize
    - input.rs — Window/input handling (WASD, camera orbit, HUD toggles)
    - update.rs — CPU updates (player/camera, AI facing, skinning palettes, FX)

- server/
  - mod.rs — In‑process server scaffold: authoritative NPC state (positions/health) and projectile collision/damage resolution. Designed to move into its own crate/process in a future workspace split.

Gameplay wiring (server‑authoritative)
- A local server (in‑process) runs NPC AI, wizard casting, projectiles, and damage.
- The client sends input/cast commands (ClientCmd) and renders snapshots (ActorSnapshot v2) from the server. The renderer never mutates game state.
- Health bars/HUD derive from replicated HP only.
- Projectile tuning (speed/life/radius/damage) is server-only. Clients send intent only; the server resolves params from `data_runtime::specs::projectiles`.
  - types.rs — GPU‑POD buffer types and vertex layouts (Globals/Model/Vertex/Instance/Particles).
    - `Globals` now includes: `sun_dir_time` (xyz + day_frac), `sh_coeffs[9]` (RGB irradiance SH‑L2 as vec4 RGB+pad), and `fog_params` (rgb + density).
  - mesh.rs — CPU mesh builders (plane, cube) → vertex/index buffers.
  - pipeline.rs — Shader/bind group layouts and pipelines (base/instanced/particles/wizard).
  - shader.wgsl — Main WGSL shaders (plane/instanced/skinned/particles). Uses directional sun + SH ambient.
  - present.wgsl — Fullscreen present: applies exponential fog and ACES-approx tonemap.
  - (moved) Standalone viewers live under `tools/` crates. Use `tools/model-viewer` for mesh/material inspection.
  - util.rs — Small helpers (depth view, surface clamp while preserving aspect).
    - Adds `oct_encode`/`oct_decode` with unit tests for normal packing.
  - anim.rs — CPU animation sampling (palettes, per‑node global transforms, clip timing cues).
  - scene.rs — Demo scene assembly (spawn world, build instance buffers, camera target). Wizards now spawn over generated terrain. The renderer no longer spawns static ruins; the only ruins you see are streamed from the server as destructible chunk meshes.
  - material.rs — Wizard material creation (base color texture + transform uniform).
  - fx.rs — FX resources (instances buffer, model bind group, quad VB) and integration helpers.
  - zombies.rs — Skinned zombie assets/instances (server-driven ring).
  - deathknight.rs — Skinned Death Knight boss (zombie-guy.glb), single oversized instance.
  - draw.rs — Renderer draw methods for wizards, zombies, death knight, and particles.
  - ui.rs — On-screen UI overlays (nameplates/text/bars) rendered in screen space, plus a minimal HUD.

## Build & Dev Loop
- Run: `cargo run`
- Tests: `cargo test`
- Lints: `cargo clippy --all-targets -- -D warnings`
- Auto-reload: `cargo install cargo-watch` then:
  - `cargo watch -x run` (rebuild and rerun on change), or
  - `cargo dev` / `cargo dev-test` via Cargo aliases in `.cargo/config.toml`.

## Feature Flags (Renderer)
- Default build: no legacy features; `render_wgpu` does not link `server_core`.
- Deprecated legacy/demo flags (kept only for archaeology; do not use): `legacy_client_ai`, `legacy_client_combat`, `legacy_client_carve`, `vox_onepath_demo`. CI builds without them.

## Frame Graph (Renderer)
- The renderer encodes pass I/O in a minimal static frame-graph (`renderer::graph`).
- Invariants validated each frame:
  - A pass may not sample from a resource it writes this frame.
  - Depth is read-only in passes.
- Passes and I/O:
  - sky: writes SceneColor
  - main: reads Depth, writes SceneColor
  - blit_scene_to_read: reads SceneColor, writes SceneRead (when not direct-present)
  - ssr: reads Depth + SceneRead, writes SceneColor
  - ssgi: reads Depth + SceneRead, writes SceneColor
  - post_ao: reads Depth, writes SceneColor
  - bloom: reads SceneRead, writes SceneColor

- sim_core/
  - rules/ — SRD rules helpers (`attack`, `dice`, `saves`).
  - combat/ — Combat model (`fsm`, `damage`, `conditions`).
  - sim/ — Headless sim engine (components, systems, state, runner, rng, scheduler, events, types).
    - systems/cast_begin.rs — Validates GCD/cooldowns; starts casts and kicks off cooldowns.
    - systems/damage.rs — Applies damage with THP absorption; triggers Concentration checks (DC = max(10, floor(dmg/2))).
    - systems/buffs.rs — Bless/Heroism; Concentration start/end; THP grant.
