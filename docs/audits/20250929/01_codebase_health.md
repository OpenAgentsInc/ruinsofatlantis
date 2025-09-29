# Codebase Health Summary

Repository Structure
- Workspace members clearly separated (renderer, sim, data, platform, tools). Root remains a thin app shell with `src/main.rs` calling the platform runner (`src/main.rs:1`).
- Renderer modules live under `crates/render_wgpu/src/gfx/**` and are re‑exported in the root shell (`src/gfx/mod.rs:3`).

Crate Health (quick read)
- render_wgpu: Feature‑rich prototype renderer; orchestration spread across large modules (`crates/render_wgpu/src/gfx/renderer/*.rs`). CPU helpers (camera, terrain, hiz, util) have tests.
- sim_core: Deterministic engine with sensible modularity (`rules/*`, `combat/*`, `sim/*`); tests robust via integration harness.
- data_runtime: Clean serde models, loader utilities, schema directory; zone manifests schema’ed; spells/classes serde‑validated only.
- platform_winit: Minimal and correct use of `winit` 0.30; headless guard present (`crates/platform_winit/src/lib.rs:78`).
- shared/assets: Consolidated GLTF/Draco loaders and CPU types; fallback logic exists for Draco JSON path; good separation from renderer.
- tools: Useful suite (`model-viewer`, `zone-bake`, `gltf-decompress`, `sim-harness`) — captured in workspace.

Ownership & Boundaries
- Boundaries are respected across crates; re‑exports in `render_wgpu` are explicit and avoid deep app coupling (`crates/render_wgpu/src/lib.rs:15`).
- Some leaky concerns remain: collision/static coupling appears in renderer update path; consider minimal traits to invert that dependency.

Docs & Comments
- Module headers present in most crates; `src/README.md:1` accurately describes the shell and controls.
- Renderer’s submodules would benefit from local docblocks describing data flow and lifetime for attachments and BGLs.

Code Style & Naming
- Consistent Rust edition, namespacing, and imports. A few large files break readability in `renderer/*`. Prefer smaller focused files per subsystem.

Dependency Hygiene
- `AGENTS.md` mandates `cargo add/rm/upgrade` — good. Consider adding automated checks (`cargo deny`) and MSRV/toolchain pinning.

Test Coverage
- Strong in `tests/` for sim/data; spotty for renderer orchestration and tools.
- Renderer CPU‑only modules (terrain, hiz, util) have unit tests; expand to orchestration/graph tests using CPU hashing of buffer content.

Primary Gaps
1) Orchestration complexity in renderer; unclear lifetimes & rebuild rules.
2) Stringly identifiers in sim; implicit fallback paths to disk.
3) Incomplete schema coverage and lack of golden tests for packs.
4) CI doesn’t enforce WGSL validation or dep policy.

