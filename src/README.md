# Source Layout Overview

This document summarizes the `src/` folder structure and what each module does.

- lib.rs — Crate root; re‑exports main modules.
- main.rs — Binary entry; sets up logging and runs the winit platform loop.
- platform_winit.rs — Window/event loop integration using winit 0.30.

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
  - camera_sys.rs — Orbit camera + `Globals` assembly for billboarding.
  - types.rs — GPU‑POD buffer types and vertex layouts (Globals/Model/Vertex/Instance/Particles).
  - mesh.rs — CPU mesh builders (plane, cube) → vertex/index buffers.
  - pipeline.rs — Shader/bind group layouts and pipelines (base/instanced/particles/wizard).
  - shader.wgsl — Main WGSL shaders (plane/instanced/skinned/particles).
  - shader_wizard_viewer.wgsl — WGSL for standalone wizard viewer bin.
  - util.rs — Small helpers (depth view, surface clamp while preserving aspect).
  - anim.rs — CPU animation sampling (palettes, per‑node global transforms, clip timing cues).
  - scene.rs — Demo scene assembly (spawn world, build instance buffers, camera target).
  - material.rs — Wizard material creation (base color texture + transform uniform).
  - fx.rs — FX resources (instances buffer, model bind group, quad VB) and integration helpers.
  - draw.rs — Renderer draw methods for wizards and particles.
  - ui.rs — On-screen UI overlays (nameplates/text) rendered in screen space.

- sim/
  - mod.rs — Sim engine index and exports.
  - components/ — Sim ECS component types (controller, statuses, projectiles, threat, etc.).
  - systems/ — Sim systems (AI, cast begin/progress, saves, damage, buffs, projectiles, input).
  - state.rs — SimState (RNG, actors, spell cache, pending effects, logs, environment flags).
  - runner.rs — Scenario runner and loop (ticks systems, checks win/loss).
  - rng.rs — RNG setup; deterministic streams.
  - scheduler.rs — Tick scheduling helpers.
  - events.rs — Event definitions used across systems.
  - types.rs — Small shared sim types.
