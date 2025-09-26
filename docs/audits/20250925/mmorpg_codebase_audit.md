# Ruins of Atlantis — Codebase Audit (2025-09-25)

Commit: 8f6abd2 (working tree has local modifications in src/assets/mod.rs, src/gfx/mod.rs)
Scope: Full repo with emphasis on Rust source under `src/`

## Executive Summary

- Overall alignment with large-MMO single-repo best practices: ~60% sound, ~40% shortcuts/smells to address.
- Strengths: Clear module boundaries for core/sim/ecs/gfx, helpful docblocks, robust GLTF handling, deterministic sim with SRD-driven data, modern winit/wgpu usage.
- Key risks: Two monoliths (`src/gfx/mod.rs` ~1008 LOC, `src/assets/mod.rs` ~940 LOC) accumulate disparate responsibilities; runtime Draco decompression contradicts repo policy; zero tests; viewer/debug paths leak into runtime; edition pinned to 2024 may strain toolchains.
- Suggested path: Split rendering/animation/assets responsibilities, add tests for core/sim, move probes to a `tools/` crate, gate debug IO behind features, and plan a workspace split (`client/`, `server/`, `shared/`, `tools/`).

Indicative LOC (via ripgrep):
- gfx: 1689, assets: 940 (≈ 62.7% of src); sim + core: 1050 (≈ 25%); bins: 420 (≈ 10%).

## What’s Solid

- Cohesive module layout with clear intent
  - `src/core/` (SRD-aligned data, rules, combat FSM) with docblocks. Example: `src/core/combat/fsm.rs:1`.
  - `src/sim/` (deterministic engine, systems pipeline) separated from rendering. Example: `src/sim/mod.rs:1`.
  - `src/gfx/` split into camera/mesh/pipeline/types/util with focused responsibilities. Example: `src/gfx/types.rs:1`.
  - Minimal ECS scaffold (`src/ecs/mod.rs:1`) adequate for prototype instance generation.
- Rendering fundamentals
  - Explicit GPU-POD types using `bytemuck` and explicit vertex layouts (`src/gfx/types.rs:13`), careful depth usage (`src/gfx/util.rs:16`).
  - Modern `winit` 0.30 `ApplicationHandler` integration (`src/platform_winit.rs:15`), proper surface caps handling and present mode selection (`src/gfx/mod.rs:74`).
  - WGSL shaders are tidy, with a distinct skinned path and sane attribute layout (`src/gfx/shader.wgsl:60`).
- Assets pipeline robustness
  - GLTF loader handles skinned attributes, inverse bind, parent maps, and animations (`src/assets/mod.rs:28`).
  - Native Draco decode path for JSON extension and fallbacks are implemented (`src/assets/mod.rs:571`).
  - Separate one-time decompression helper binary (`src/bin/gltf_decompress.rs:1`).
- Simulator is pragmatic and deterministic
  - SRD-driven `SpellSpec` and simple FSM with GCD, cast/channel/recovery (`src/core/data/spell.rs:18`, `src/core/combat/fsm.rs:9`).
  - System pipeline mirrors MMO concerns (AI, cast begin, saves, attack rolls, damage, conditions) with clean data flow via `SimState` queues (`src/sim/runner.rs:1`, `src/sim/systems/attack_roll.rs:1`).
- Documentation hygiene
  - Module docblocks are present and helpful across new modules.
  - Design docs in `docs/` and SRD mirror under `docs/srd/` show good process hygiene.

## Shortcuts and Code Smells

1) Monolithic renderer doing too much (~1008 LOC)
- `Renderer` owns GPU state but also:
  - Builds an ECS world, spawns entities, and decides camera targets (`src/gfx/mod.rs:339`).
  - Loads GLTF, samples animations, derives strike timings, manages projectile/particle pools, and data-driven VFX (`src/gfx/mod.rs:560` onward).
- Impact: Cross-layer coupling of rendering, scene, animation, and gameplay/VFX logic. Hinders testability and future feature growth (e.g., networking, streaming, LODs).

2) Monolithic assets module (~940 LOC) with mixed responsibilities
- Combines: unskinned/skinned GLTF, Draco JSON-path decode, base color extraction, animation track construction, plus an at-runtime decompression fallback.
- Impact: Hard to reason about, risky to modify, and contradicts policy (see next item).

3) Runtime Draco decompression contradicts repo policy
- `prepare_gltf_path()` auto-runs `gltf-transform` via `npx`/global on import failure (`src/assets/mod.rs:884`). AGENTS.md explicitly states runtime does NOT attempt decompression.
- Impact: Startup side effects, toolchain dependency (Node), failure modes masked by side-effecting fixups. Should be a strict offline step.

4) Zero tests (unit or integration)
- No `#[cfg(test)]` modules and no `tests/` crate content were found.
- Impact: Refactors carry high risk; no enforced invariants for core/sim rules, FSM, or loaders.

5) Edition set to 2024
- `Cargo.toml:5` uses `edition = "2024"`. While forward-looking, this may break builds for default stable toolchains and disagrees with guideline “Rust 2021+”.

6) Viewer/diagnostic code leaks into runtime paths
- Debug image dumps (`data/debug/*.png`) and diagnostic logs occur in the main renderer (`src/gfx/mod.rs:611`), not gated by features.
- Impact: Extra deps and IO in hot paths; noisy logs and filesystem writes in release runs.

7) Duplicated functionality across bins and runtime
- Local copies of util (e.g., `scale_to_max`) and depth creation exist in the `wizard_viewer` bin (`src/bin/wizard_viewer.rs:151`), duplicating `src/gfx/util.rs`.
- Impact: Divergence risk; minor but signals missing shared “viewer support” layer under `tools/`.

8) Unused dependency
- `draco` crate present in Cargo.toml but not referenced; code uses `draco_decoder` (`Cargo.toml:13`, search shows no `use draco`).
- Impact: Build weight and maintenance overhead.

9) Configuration and magic numbers
- Plane size, FX capacity, ring radii, animation selections are hard-coded (`src/gfx/mod.rs:248`, `src/gfx/mod.rs:409`, `src/gfx/mod.rs:436`).
- Impact: Harder to tune and to test; should migrate into config or scenario data.

10) Renderer handles animation sampling and skin palettes
- CPU-animated palette generation lives in gfx; palette upload and instance palette-base assignment are mixed with scene creation.
- Impact: Limits reuse with server-side or headless sim tooling; makes animation unit testing awkward.

## Recommendations (Prioritized)

1) Tighten policy adherence and boundaries
- Remove runtime auto-decompression. Make `prepare_gltf_path()` a no-op selection (prefer `*.decompressed.gltf`), and delegate decompression to the helper bin. Update renderer callsites accordingly.
- Gate debug IO (image dumps, verbose logs) behind a `debug-io` cargo feature. Default off for release.

2) Split monoliths by responsibility
- `src/assets/` into submodules:
  - `assets/gltf.rs` (generic read + buffers/images access)
  - `assets/draco.rs` (JSON extension decode helpers)
  - `assets/skinning.rs` (skinned mesh + animation clip construction)
  - `assets/texture.rs` (CPU texture extraction/conversion)
- `src/gfx/` refactor:
  - Keep `Renderer` focused on GPU surfaces, pipelines, bind groups, and draws.
  - Move scene assembly, camera target selection, and instance generation to a `scene` module (client-side only).
  - Move animation sampling and palette construction to `anim` (CPU) producing storage buffers for gfx.
  - Move projectile/particle systems out of gfx into sim/client systems, emitting renderable instances.

3) Establish tests around the stable core
- Unit tests:
  - FSM ticks and transitions (`src/core/combat/fsm.rs`) — cast/channel/recovery and GCD gating.
  - Rules helpers (saves/attack resolve) — deterministic RNG injection points.
  - Data loaders — JSON roundtrips on representative fixtures (spells/classes/monsters).
- Integration tests:
  - Sim scenario runner (e.g., `data/scenarios/example.yaml`) asserting terminal conditions deterministically.

4) Prepare workspace transition (medium term)
- Create workspace with crates: `client/`, `server/`, `shared/`, `tools/`.
  - `shared/`: today’s `core/` and any cross-cutting math and schemas.
  - `client/`: `gfx/`, `assets/`, client-facing ECS/scene/anim systems.
  - `server/`: headless sim + net scaffolding.
  - `tools/`: `wizard_viewer`, `bevy_probe`, `gltf_decompress`, importers/exporters.
- This unlocks lighter client/server binaries and cleaner dependency graphs (e.g., no `bevy` in client if not needed).

5) Toolchain and dependency hygiene
- Set `edition = "2021"` now; re-evaluate 2024 when stable across dev machines/CI.
- Remove unused `draco` crate; keep `draco_decoder` if needed.
- Add `cargo clippy -- -D warnings` to CI; fix or annotate any unavoidable lints.

6) Configuration and data
- Hoist demo constants into config files or scenario authoring (plane size, ring layout, FX caps, cam orbit params). Provide `config/` defaults and document env overrides.

7) Performance sanity checks (later)
- Batch palette uploads and instance updates; ensure minimal per-frame CPU allocations.
- Investigate storage buffer sizing/updates (e.g., double-buffering, partial writes).

## Alignment Scorecard (by area)

- Core data/rules/combat (src/core): Strong (80–90%)
  - Clear schemas and SRD separation. FSM is clean and testable. Lacks tests today.
- Simulator (src/sim): Strong (75–85%)
  - Deterministic, modular systems; logs/data flows are reasonable. Move toward integration tests and configurable policies.
- Rendering (src/gfx): Mixed (50–60%)
  - Good low-level structure, but `Renderer` takes on scene, anim, VFX, and data concerns. Needs splitting and feature gating for debug IO.
- Assets (src/assets): Mixed (50–60%)
  - Robust functionality, but monolithic and policy-contradicting runtime decompress. Split modules and remove side-effecting prep.
- Binaries/tools (src/bin): OK for now (60–70%)
  - Useful probes, but should migrate to `tools/` crate and reuse shared utilities to reduce duplication.
- Build/tooling: Needs tightening (50–60%)
  - Edition 2024, no tests, potential unused deps. Add CI clippy/fmt/test gates.

## Concrete Next Steps (2–3 weeks)

Week 1
- Change edition to 2021, remove unused `draco` dep, add `features = ["debug-io"]` for renderer diagnostics.
- Delete runtime decompression from `prepare_gltf_path()` or gate it entirely behind a non-default `asset-decompress` feature; default to strict mode using `*.decompressed.gltf` only.
- Add unit tests for FSM and spell loader; add a minimal integration test that runs `sim::runner` on `example.yaml` and asserts non-panics and a terminal state.

Week 2
- Split `assets/mod.rs` into `gltf.rs`, `draco.rs`, `skinning.rs`, `texture.rs` (no behavior changes). Update callsites.
- Move scene assembly (world spawn, camera target, instance lists) out of `Renderer` into a `scene` module; `Renderer` receives prepared instance buffers and palettes.

Week 3
- Add `tools/` crate and move `wizard_viewer`, `bevy_probe`, `gltf_decompress` there. Deduplicate util functions by exposing a small shared helper crate (or `shared/` subcrate) if warranted.
- Add CI with fmt/clippy/test gates.

## Notable File References

- Renderer monolith and cross-responsibilities: `src/gfx/mod.rs:1`, `src/gfx/mod.rs:339`, `src/gfx/mod.rs:560`.
- Assets runtime decompression (policy violation): `src/assets/mod.rs:884`.
- Edition setting: `Cargo.toml:5`.
- Debug IO in runtime renderer: `src/gfx/mod.rs:611`.
- Duplicate utility pattern in viewer: `src/bin/wizard_viewer.rs:151`.
- Unused `draco` dep (present, not referenced): `Cargo.toml:13`.

## Closing Notes

This is a strong prototype foundation with a sensible split between shared rules/sim and client rendering. The main work now is ungluing responsibilities that have accumulated in two large modules, aligning the assets policy, and adding tests. Doing this before adding networking, streaming, or persistence will save significant time and avoid architectural repainting later.

---

Addendum (2025-09-26): Remove Runtime Decompression and Policy Clarification

- What changed
  - Disabled runtime auto-decompression in `prepare_gltf_path()` so the loader no longer spawns `npx`/`gltf-transform` or probes imports. It now simply prefers a sibling `*.decompressed.gltf` if present, otherwise returns the original path. File: `src/assets/mod.rs:884` (updated function body).
- Why it was a policy violation
  - AGENTS.md states that the runtime does NOT attempt Draco decompression; decompression must be a one-time, offline step (e.g., via `cargo run --bin gltf_decompress -- <in> <out>`). The prior implementation attempted to call external tools at runtime on import failure, introducing side effects, build/runtime environment coupling (Node required), and masking asset preparation issues. This conflicts with the documented policy and good prod practices for predictable startup.
- Follow-ups (recommended)
  - Optionally remove now-unused helper `try_gltf_transform_decompress()` or gate it behind a `dev-tools` feature in a future `tools/` crate.
  - Ensure any asset referencing `KHR_draco_mesh_compression` has a checked-in `*.decompressed.gltf` or is re-exported without Draco.

Addendum (2025-09-26): Edition Clarification and Helper Removal

- AGENTS.md now explicitly states we target Rust 2024 edition.
- Removed the unused runtime helper `try_gltf_transform_decompress` from `src/assets/mod.rs` and eliminated any call sites. Runtime no longer attempts decompression; use the `gltf_decompress` tool or a pre‑decompressed asset alongside the original.

Addendum (2025-09-26): Assets Module Refactor

- What changed
  - Broke up the monolithic `src/assets/mod.rs` into focused submodules:
    - `src/assets/types.rs` — CPU types (CpuMesh, SkinnedMeshCPU, AnimClip, Tracks, TextureCPU).
    - `src/assets/gltf.rs` — unskinned GLTF mesh loading and JSON+Draco fallback path.
    - `src/assets/skinning.rs` — skinned mesh + animation clip loading.
    - `src/assets/draco.rs` — internal Draco decode helpers used by the loaders.
    - `src/assets/util.rs` — `prepare_gltf_path()` and related helpers.
  - `src/assets/mod.rs` now re-exports the public API so existing imports like `use crate::assets::{load_gltf_mesh, load_gltf_skinned, AnimClip, SkinnedMeshCPU};` keep working.
- Scope and intent
  - This is a structural change only; behavior is preserved except for the prior policy fix (no runtime decompression).
  - The split improves readability, isolates Draco-specific code, and makes future testing/ownership clearer.
- Follow-ups (recommended)
  - Add unit tests around `gltf::load_gltf_mesh` (Draco JSON fallback) and `skinning::load_gltf_skinned` (clip extraction, inverse bind handling).
  - Consider lifting `CpuMesh` off `gfx::Vertex` to a pure CPU vertex type with an adapter into `gfx` to reduce coupling.

Addendum (2025-09-26): Unit Tests Added (Assets)

- Added unit tests co-located with the loaders:
  - `src/assets/gltf.rs`:
    - `load_gltf_mesh_wizard` — loads `assets/models/wizard.gltf`; asserts non-empty vertices/indices.
    - `load_gltf_mesh_ruins_draco` — loads `assets/models/ruins.gltf` (Draco); asserts non-empty vertices/indices.
  - `src/assets/skinning.rs`:
    - `load_gltf_skinned_wizard` — loads `assets/models/wizard.gltf`; asserts skinned vertices/indices/joints and non-empty animations.
  - `src/assets/util.rs`:
    - `returns_importable_path` — `prepare_gltf_path` returns a path importable by `gltf::import` (prefers original if valid; otherwise a sibling `*.decompressed.gltf`).
- Rationale: Establish minimal invariants for loaders while keeping runtime side effects out of tests. These tests exercise both the standard and Draco paths deterministically using checked-in assets.
