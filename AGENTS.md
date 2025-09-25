# Repository Guidelines

## Project Structure & Module Organization
- Root: `README.md`, `GDD.md` (design), `LICENSE`, `NOTICE`.
- Rust crate (current): single binary at root with `Cargo.toml` and `src/main.rs`.
- Future workspace (planned): crates under `server/`, `client/`, `shared/`, `tools/`.
- Assets: `assets/` (art/audio), `data/` (JSON/CSV), `docs/` (design notes, diagrams).
- Tests: unit tests within each crate’s `src/`; integration tests in top‑level `tests/`.

### Code Organization (client prototype)
- Rendering lives under `src/gfx/` and is split by responsibility:
  - `gfx/mod.rs`: `Renderer` entry point (init/resize/render) and high‑level wiring.
  - `gfx/camera.rs`: camera math and helpers.
  - `gfx/types.rs`: GPU‑POD buffer types and vertex layouts (`Globals`, `Model`, `Vertex`, `Instance`).
  - `gfx/mesh.rs`: CPU‑side mesh builders (cube, plane) → vertex/index buffers.
  - `gfx/pipeline.rs`: shader load, bind group layouts, pipelines (base/instanced/wireframe).
  - `gfx/shader.wgsl`: WGSL shaders for plane + instanced draws.
  - `gfx/util.rs`: small helpers (depth view, surface clamp preserving aspect).
- Going forward, keep modules cohesive, focused, and documented; do not accrete new features into monolith files. Add sub‑modules as systems grow (input, scene graph, streaming, UI, net, ECS, etc.).

### Documentation & Comments
- All new modules must start with a brief docblock explaining scope and how to extend it.
- Add inline comments for non‑obvious math, layout decisions, and API quirks (e.g., WGSL/std140 padding, wgpu limits).
- Prefer doc comments (`///`) on public types/functions so `cargo doc` is useful.

## Game Design Document (GDD)
- Canonical design source: `GDD.md` at repo root.
- Keep these sections current: Philosophy, Game Mechanics, SRD Usage and Attribution, SRD Scope & Implementation, and any Design Differences from SRD.
- Before gameplay/rules changes, update `GDD.md` in the same PR; explain rationale and SRD impact.
- SRD usage: maintain exact attribution in `NOTICE`; document any terminology deviations (e.g., using “race”).

## Build, Test, and Development Commands
- Prerequisites: Install Rust via `rustup` (stable toolchain). If the edition is unsupported, run `rustup update`.
- Run the app (root crate): `cargo run`
- Build (debug/release): `cargo build` / `cargo build --release`
- Run with logs: `RUST_LOG=info cargo run`
- Tests: `cargo test`
- Format/lint: `cargo fmt` and `cargo clippy -- -D warnings`
- Optional dev loop: `cargo install cargo-watch` then `cargo watch -x run`

## Assets & GLTF
- Place models under `assets/models/` (e.g., `assets/models/wizard.gltf`).
- GLTF loader uses `gltf` crate with the `import` feature, so external buffers/images resolve via relative paths. Keep referenced files next to the `.gltf` or adjust URIs accordingly.
- Current prototype loads two meshes (`wizard.gltf`, `ruins.gltf`) and draws them via instancing.
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

## Build Hygiene
- Always leave the repo in a compiling state before stopping work.
- Run `cargo check` (and for changed binaries, `cargo build`) to confirm there are no compile errors.
- Prefer to address warnings promptly, but errors are never acceptable at handoff.

## Coding Style & Naming Conventions
- Rust 2021+, 4‑space indent; target ~100‑char lines.
- Names: snake_case (functions/files), CamelCase (types/traits), SCREAMING_SNAKE_CASE (consts).
- Prefer explicit imports; avoid wildcards. Document public APIs with rustdoc.
- Use `rustfmt` (enforced) and `clippy`; fix warnings or add justified `#[allow]` with a comment.

## Testing Guidelines
- Unit tests co‑located via `#[cfg(test)]` modules.
- Integration tests in `tests/` with descriptive names (e.g., `combat_turns_test.rs`).
- Keep tests deterministic; gate network/time‑sensitive cases behind feature flags.

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
