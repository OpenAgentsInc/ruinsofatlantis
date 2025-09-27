# Source Layout Overview

This document summarizes the `src/` folder structure and what each module does.

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
  - mod.rs — Core facade for data, rules, and combat.
  - data/
    - mod.rs — Data module index.
    - ids.rs — Strongly typed IDs for data records.
    - ability.rs — Ability schema (prototype).
    - class.rs — Class schema used by sim defaults.
    - monster.rs — Monster schema used by sim defaults.
    - spell.rs — SRD‑aligned SpellSpec used by tools and the sim.
    - scenario.rs — Scenario schema for the sim harness.
    - loader.rs — Data file readers for JSON under `data/`.
  - rules/
    - mod.rs — SRD rules index.
    - attack.rs — Advantage enum and attack scaffolding.
    - dice.rs — Dice helpers (policy/crit hooks).
    - saves.rs — Saving throw kinds and helpers.
  - combat/
    - mod.rs — Combat model index.
    - fsm.rs — Action FSM (cast/channel/recovery) and GCD.
    - damage.rs — Damage plumbing (prototype).
    - conditions.rs — Basic conditions.

- ecs/
  - mod.rs — Minimal ECS scaffolding (entities, transforms, render kinds).

- gfx/
  - mod.rs — Renderer entry (init/resize/render) and high‑level wiring.
  - camera.rs — Camera type and view/projection math.
  - camera_sys.rs — Orbit and third‑person follow camera helpers + `Globals`.
  - terrain.rs — Seeded heightmap terrain generation and simple woodland scatter (Phase 1).
  - sky.rs — Hosek–Wilkie sky state on CPU (time‑of‑day, sun dir, SH ambient) and uniform packing.
  - sky.wgsl — Background sky pass (fullscreen triangle) evaluating HW from CPU‑provided params.
  - Player casting: press `1` to trigger the PC's `PortalOpen` animation; 1.5s after start spawns a Fire Bolt forward. The renderer queues the cast on key press and advances the PC animation, reverting to `Still` after the clip completes.
  - ui.rs — Overlays: nameplates (text atlas) and health bars (screen‑space quads with green→yellow→red gradient).
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
  - anim.rs — CPU animation sampling (palettes, per‑node global transforms, clip timing cues).
  - scene.rs — Demo scene assembly (spawn world, build instance buffers, camera target). Wizards now spawn over generated terrain.
  - material.rs — Wizard material creation (base color texture + transform uniform).
  - fx.rs — FX resources (instances buffer, model bind group, quad VB) and integration helpers.
  - draw.rs — Renderer draw methods for wizards and particles.
  - ui.rs — On-screen UI overlays (nameplates/text) rendered in screen space.

## Build & Dev Loop
- Run: `cargo run`
- Tests: `cargo test`
- Lints: `cargo clippy --all-targets -- -D warnings`
- Auto-reload: `cargo install cargo-watch` then:
  - `cargo watch -x run` (rebuild and rerun on change), or
  - `cargo dev` / `cargo dev-test` via Cargo aliases in `.cargo/config.toml`.

- sim/
  - mod.rs — Sim engine index and exports.
  - components/ — Sim ECS component types (controller, statuses, projectiles, threat, etc.).
  - systems/ — Sim systems (AI, cast begin/progress, saves, damage, buffs, projectiles, input).
    - cast_begin.rs — Validates GCD and per-ability cooldowns; starts casts and kicks off cooldowns.
    - damage.rs — Applies damage with Temporary Hit Points (THP) absorption and triggers Concentration checks (DC = max(10, floor(damage/2)), cap 30).
    - buffs.rs — Handles Bless/Heroism prototypes; starts/ends Concentration and grants THP (non-stacking: keeps higher value).
  - state.rs — SimState (RNG, actors, spell cache, pending effects, logs, environment flags).
  - runner.rs — Scenario runner and loop (ticks systems, checks win/loss).
  - rng.rs — RNG setup; deterministic streams.
  - scheduler.rs — Tick scheduling helpers.
  - events.rs — Event definitions used across systems.
  - types.rs — Small shared sim types.
