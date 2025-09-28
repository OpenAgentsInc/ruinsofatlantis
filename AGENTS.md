# Repository Guidelines

## Project Structure & Module Organization
- Root: `README.md`, `GDD.md` (design), `LICENSE`, `NOTICE`.
- Workspace: multiple crates under `crates/` and `shared/`; root remains the app shell and bins.
- Assets: `assets/` (art/audio), `data/` (JSON/CSV), `docs/` (design notes, diagrams).
- Tests: unit tests within each crate’s `src/`; integration tests in top‑level `tests/`.

### Workspace Crates (current)
- `crates/render_wgpu` — Renderer crate. Hosts the full renderer under `src/gfx/**` (camera, pipelines, shaders, UI, scene build, terrain, sky, etc.). The root `src/gfx/mod.rs` is a thin re‑export of this crate.
- `crates/platform_winit` — Platform/window/input loop (winit 0.30) that drives the renderer. Root app calls `platform_winit::run()`.
- `crates/sim_core` — Rules/combat FSM and headless sim engine (`rules/*`, `combat/*`, `sim/*`). Root re‑exports as `crate::sim` and `crate::core::{rules,combat}`.
- `crates/data_runtime` — SRD‑aligned data schemas + loaders (replaces `src/core/data`). Root re‑exports as `crate::core::data`.
- `crates/ux_hud` — HUD logic/toggles (`HudModel`), separated from renderer draw code.
- `shared/assets` — Asset loaders/types (GLTF/Draco helpers) used by tools and the renderer.

### What lives in `src/` and why
- `src/main.rs` — App entry; initializes logging and calls `platform_winit::run()`.
- `src/gfx/mod.rs` — Thin re‑export of `render_wgpu::gfx` so existing `crate::gfx` paths continue to work in the app.
- `src/core/mod.rs` — Facade re‑exports: `data_runtime` (as `crate::core::data`) and `sim_core::{rules, combat}` for compatibility.
- `src/server/**` — Prototype in‑process server NPC/state and collision. Stays here until we split a proper server crate/process.
- `src/client/**` — Gameplay/controller glue for the app. The renderer crate contains minimal shims to avoid cycles; the canonical code stays here and can be moved to a `client/` crate later when stabilized.
- `src/assets/**` — Facade re‑export over `shared/assets` so app code can use `crate::assets::*`.
- `src/bin/**` — Standalone tools and viewers (e.g., `wizard_viewer`, `gltf_decompress`, `image_probe`, `bevy_probe`). These can keep direct deps (wgpu, gltf, image) in the root.

Guidance: new reusable systems (renderer modules, platform, data, sim, HUD logic, tools libraries) should live in dedicated crates. App glue, small bins, and short‑lived prototypes can remain in `src/` until they harden, at which point prefer moving them under `crates/`.

### Code Organization (renderer)
- Rendering lives under `crates/render_wgpu/src/gfx/` split by responsibility (camera, types, mesh, pipeline, shaders, ui, terrain, sky, temporal, framegraph helpers). The root `src/gfx/mod.rs` re‑exports this for compatibility.
- Keep modules cohesive and well‑documented; prefer adding focused sub‑modules as systems grow (scene graph, streaming, UI, net, ECS, etc.).

### Documentation & Comments
- All new modules must start with a brief docblock explaining scope and how to extend it.
- Add inline comments for non‑obvious math, layout decisions, and API quirks (e.g., WGSL/std140 padding, wgpu limits).
- Prefer doc comments (`///`) on public types/functions so `cargo doc` is useful.
- Do not add meta comments like "(unused helper removed)" or "(logs removed)". If code is unused, delete it; keep comments focused on behavior and intent, not change notes.

IMPORTANT: Keep `src/README.md` current
- Whenever you add, move, or significantly change files under `src/`, immediately update `src/README.md` to reflect the real file/folder hierarchy and module responsibilities.
- Document new pipelines, shaders, UI overlays, systems, and data flows so future contributors can navigate quickly.

## Game Design Document (GDD)
- Canonical design source: `GDD.md` at repo root.
- Keep these sections current: Philosophy, Game Mechanics, SRD Usage and Attribution, SRD Scope & Implementation, and any Design Differences from SRD.
- Before gameplay/rules changes, update `GDD.md` in the same PR; explain rationale and SRD impact.
- SRD usage: maintain exact attribution in `NOTICE`; document any terminology deviations (e.g., using “race”).

## Build, Test, and Development Commands
- Prerequisites: Install Rust via `rustup` (stable toolchain). If the edition is unsupported, run `rustup update`.
- Run the app (root crate): `cargo run` (default-run is `ruinsofatlantis`)
- Build (debug/release): `cargo build` / `cargo build --release`
- Run with logs: `RUST_LOG=info cargo run`
- Tests: `cargo test`
- Format/lint: `cargo fmt` and `cargo clippy -- -D warnings`
- Optional dev loop: `cargo install cargo-watch` then `cargo watch -x run`

NOTE FOR AGENTS
- Do NOT run the interactive application during automation (e.g., avoid invoking `cargo run`). The user will run the app locally. Limit yourself to building, testing, linting, and file operations unless explicitly asked otherwise.

## Assets & GLTF
- Place models under `assets/models/` (e.g., `assets/models/wizard.gltf`).
- GLTF loader uses `gltf` crate with the `import` feature, so external buffers/images resolve via relative paths. Keep referenced files next to the `.gltf` or adjust URIs accordingly.
- Current prototype loads two meshes (`wizard.gltf`, `ruins.gltf`) and draws them via instancing.
- If a model is Draco-compressed (e.g., `ruins.gltf`), prepare a decompressed copy once:
  - `cargo run --bin gltf_decompress -- assets/models/ruins.gltf assets/models/ruins.decompressed.gltf`
  - The runtime prefers `*.decompressed.gltf` if present.
- When adding dependencies for loaders or formats, use `cargo add` (never hand‑edit `Cargo.toml`).
- Draco compression: The runtime does NOT attempt decompression. If a model declares `KHR_draco_mesh_compression`, run our one-time helper:
  - `cargo run --bin gltf_decompress -- assets/models/foo.gltf assets/models/foo.decompressed.gltf`
  - Or manually: `npx -y @gltf-transform/cli draco -d <in.gltf> <out.gltf>` (older CLIs use `decompress`).
  - Or re-export the asset without Draco. Our runtime does not decode Draco in-process.

## Dependency Management
- Never add dependencies by hand in `Cargo.toml`.
- Always use Cargo’s tooling so versions/resolution are correct:
  - Install: `cargo install cargo-edit`
  - Add deps: `cargo add <crate> [<crate>...]` (e.g., `cargo add winit wgpu log env_logger anyhow`)
  - Remove deps: `cargo rm <crate>`
- Upgrade: `cargo upgrade` (review diffs before committing)

NOTE: Strict policy — agents must not hand‑edit Cargo.toml. If a dependency change is required, use `cargo add`/`cargo rm`/`cargo upgrade`. If a file was modified manually earlier, reapply the change with Cargo tooling in the same PR.

## Build Hygiene
- Always leave the repo in a compiling state before stopping work.
- Run `cargo check` (and for changed binaries, `cargo build`) to confirm there are no compile errors.
- Prefer to address warnings promptly, but errors are never acceptable at handoff.

## Coding Style & Naming Conventions
- Rust 2024 edition, 4‑space indent; target ~100‑char lines.
- Names: snake_case (functions/files), CamelCase (types/traits), SCREAMING_SNAKE_CASE (consts).
- Prefer explicit imports; avoid wildcards. Document public APIs with rustdoc.
- Use `rustfmt` (enforced) and `clippy`; fix warnings or add justified `#[allow]` with a comment.

## Testing Guidelines
- Unit tests co‑located via `#[cfg(test)]` modules.
- Integration tests in `tests/` with descriptive names (e.g., `combat_turns_test.rs`).
- Keep tests deterministic; gate network/time‑sensitive cases behind feature flags.

ALWAYS add tests with new functionality
- Any new behavior (parsing, math/transform helpers, animation sampling, collision, UI vertex generation, etc.) must ship with unit tests.
- Prefer small, focused tests co‑located with the code. For renderer‑adjacent CPU work (math, CPU‑built buffers), add CPU‑only tests that don’t require a GPU device.
- PRs that introduce features without tests should be considered incomplete.

## Commit & Pull Request Guidelines
- Commit style: `<area>: <imperative summary>` (e.g., `server: add login flow`).
- Include what/why in body; link issues (e.g., `#123`).
- PRs must: describe changes, include screenshots for UI, update design docs (`GDD.md`), update `NOTICE` when SRD content is added, and pass build/fmt/clippy/tests.

## SRD, Licensing, and Attribution
- SRD 5.2.1 content is CC‑BY‑4.0. Include the exact attribution in `NOTICE` and keep GDD’s SRD section accurate.
- This project is D&D‑inspired/“5E compatible,” but unofficial; do not imply endorsement; avoid Wizards’ trademarks/logos.
- Keep `LICENSE` (Apache‑2.0) intact; add third‑party notices under `NOTICE`.

## Security & Configuration Tips
- Never commit secrets. Use env vars and a `.env.example`; ignore real `.env` files.
- Prefer local config under `config/` with sample defaults; document required vars in `README.md`.
