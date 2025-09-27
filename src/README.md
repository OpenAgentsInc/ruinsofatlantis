# Source Layout Overview

This document summarizes the `src/` folder structure and what each module does.

Workspace crates (added for modularization)
- crates/data_runtime — SRD-aligned data schemas + loaders (replaces `src/core/data`; re-exported under `crate::core::data`).
- crates/render_wgpu — Renderer facade crate (temporarily re-exports `crate::gfx`).
- crates/sim_core — Rules/combat/sim crate (moved from `src/core/{rules,combat}` and `src/sim`). Re-exported under `crate::core::{rules,combat}` and `crate::sim` for compatibility.
- crates/platform_winit — Platform loop facade (temporarily re-exports `crate::platform_winit`).
- crates/ux_hud — HUD logic crate (now owns perf/HUD toggles; F1 toggles perf overlay, H toggles HUD).

- Workspace crates (new)
- shared/assets — Library crate re-exporting our asset loaders for tools.
- tools/model-viewer — Standalone wgpu viewer that loads GLTF/GLB via shared/assets.

- lib.rs — Crate root; re‑exports main modules.
- main.rs — Binary entry; sets up logging and runs the winit platform loop.
- platform_winit.rs — Window/event loop integration using winit 0.30.

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

- bin/
  - bevy_probe.rs — Bevy‑based material/texture probe for the wizard asset.
  - gltf_decompress.rs — One‑time CLI to decompress Draco GLTFs (offline step).
  - image_probe.rs — Simple image IO experiments.
  - sim_harness.rs — Basic runner for the combat simulator.
  - wizard_viewer.rs — Standalone viewer rendering the wizard with a simple pipeline.
  - wizard_viewer.wgsl — WGSL shader for the standalone wizard viewer.

- core/
  - mod.rs — Core facade; re-exports `data_runtime` as `crate::core::data` and `sim_core::{rules,combat}` for compatibility.

- ecs/
  - mod.rs — Minimal ECS scaffolding (entities, transforms, render kinds).

- gfx/
  - mod.rs — Renderer entry (init/resize/render) and high‑level wiring.
  - camera.rs — Camera type and view/projection math.
  - camera_sys.rs — Orbit and third‑person follow camera helpers + `Globals`.
  - gbuffer.rs — G‑Buffer attachments and formats (albedo, normal‑oct, rough/metal, emissive, motion).
  - hiz.rs — Z‑MAX depth pyramid resources over linear R32F depth; CPU reference downsample + tests.
  - hiz.comp.wgsl — Compute shader for linearizing depth and 2×2 Z‑MAX reduction.
  - temporal/
    - mod.rs — Temporal module index.
    - reprojection.rs — CPU reprojection helpers + tests; prev/curr jitter handling.
    - history.rs — Temporal params and clamp helpers + tests.
  - terrain.rs — Seeded heightmap terrain generation and simple woodland scatter (Phase 1). Parameters (size/extent/seed) come from the active Zone manifest.
  - sky.rs — Hosek–Wilkie sky state on CPU (time‑of‑day, sun dir, SH ambient) and uniform packing.
  - sky.wgsl — Background sky pass (fullscreen triangle) evaluating HW from CPU‑provided params.
  - fullscreen.wgsl — Shared fullscreen VS: offscreen no‑flip, present Y‑flip.
  - ssgi.wgsl — Placeholder compute entry for screen‑space diffuse GI (to be implemented).
  - ssr.wgsl — Placeholder compute entry for screen‑space reflections (to be implemented).
  - shaders/common.wgsl — Shared shader constants and binding conventions.
  - Player casting: press `1` to trigger the PC's `PortalOpen` animation; 1.5s after start spawns a Fire Bolt forward. The renderer queues the cast on key press and advances the PC animation, reverting to `Still` after the clip completes.
  - ui.rs — Overlays: nameplates (text atlas), health bars (screen‑space quads with green→yellow→red gradient), and a minimal HUD (player HP + bottom hotbar with GCD overlay) for the wizard scene.
  - pipeline.rs — Adds `create_bar_pipeline` for solid‑color screen quads.
  - shader.wgsl — Adds `vs_bar`/`fs_bar` for health bar rendering.

- server/
  - mod.rs — In‑process server scaffold: authoritative NPC state (positions/health) and projectile collision/damage resolution. Designed to move into its own crate/process in a future workspace split.

Gameplay wiring (prototype)
- NPCs: multiple rings of cube NPCs spawn at various radii. They have health and can be killed; on hit, bars drop and color shifts.
- Fire Bolt: on hit, applies damage to NPCs (logs hits/deaths). Impact spawns a small particle burst.
- Health bars: shown for the player, all wizards, and all NPC boxes. Bars render above the head/center in screen space.
  - types.rs — GPU‑POD buffer types and vertex layouts (Globals/Model/Vertex/Instance/Particles).
    - `Globals` now includes: `sun_dir_time` (xyz + day_frac), `sh_coeffs[9]` (RGB irradiance SH‑L2 as vec4 RGB+pad), and `fog_params` (rgb + density).
  - mesh.rs — CPU mesh builders (plane, cube) → vertex/index buffers.
  - pipeline.rs — Shader/bind group layouts and pipelines (base/instanced/particles/wizard).
  - shader.wgsl — Main WGSL shaders (plane/instanced/skinned/particles). Uses directional sun + SH ambient.
  - shader_wizard_viewer.wgsl — WGSL for standalone wizard viewer bin.
  - util.rs — Small helpers (depth view, surface clamp while preserving aspect).
    - Adds `oct_encode`/`oct_decode` with unit tests for normal packing.
  - anim.rs — CPU animation sampling (palettes, per‑node global transforms, clip timing cues).
  - scene.rs — Demo scene assembly (spawn world, build instance buffers, camera target). Wizards now spawn over generated terrain.
  - material.rs — Wizard material creation (base color texture + transform uniform).
  - fx.rs — FX resources (instances buffer, model bind group, quad VB) and integration helpers.
  - draw.rs — Renderer draw methods for wizards and particles.
  - ui.rs — On-screen UI overlays (nameplates/text/bars) rendered in screen space, plus a minimal HUD.

## Build & Dev Loop
- Run: `cargo run`
- Tests: `cargo test`
- Lints: `cargo clippy --all-targets -- -D warnings`
- Auto-reload: `cargo install cargo-watch` then:
  - `cargo watch -x run` (rebuild and rerun on change), or
  - `cargo dev` / `cargo dev-test` via Cargo aliases in `.cargo/config.toml`.

- sim_core/
  - rules/ — SRD rules helpers (`attack`, `dice`, `saves`).
  - combat/ — Combat model (`fsm`, `damage`, `conditions`).
  - sim/ — Headless sim engine (components, systems, state, runner, rng, scheduler, events, types).
    - systems/cast_begin.rs — Validates GCD/cooldowns; starts casts and kicks off cooldowns.
    - systems/damage.rs — Applies damage with THP absorption; triggers Concentration checks (DC = max(10, floor(dmg/2))).
    - systems/buffs.rs — Bless/Heroism; Concentration start/end; THP grant.
