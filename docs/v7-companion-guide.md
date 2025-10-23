# OpenAgents v7 Companion Guide

This note summarizes the `~/code/v7` companion repository that powers the OpenAgents iOS client and Rust engine we reference for DOF-oriented work. Use it when you need to study the native app pipeline, reuse tooling, or align renderer features with Ruins of Atlantis.

## Location & Purpose
- Local path: `~/code/v7`
- Public upstream: `github.com/OpenAgentsInc/v7`
- Role: ships the App Store **OpenAgents** client and prototypes the Rust agent engine (Codex bridge, renderer, plan HUD) that we integrate with.

## Repository Layout (highlights)
- `OpenAgents/` — Xcode project. `RustCubeRenderer.swift` bridges Metal `CAMetalLayer` draw calls to the Rust renderer and wires a “Run” button to `codexd`.
- `crates/` — Rust workspace (see `Cargo.toml`). Key crates:
  - `wgpu-cube` — Rust + `wgpu` demo rendered into `CAMetalLayer`; owns FFI (`tricoder_cube_*`) and HUD overlays (`ui_hud` plan/feed panels).
  - `tricoder-engine` — Future agent engine core; currently stubs initialization and will host DOF/scene logic.
  - `codexd` — Axum server that launches `codex exec`, streams events via WebSocket (`/ws`), and exposes `/start` for ad-hoc prompts. Simulator button in Swift calls this endpoint.
  - `codex-bridge` / `codex-client` — Shared bridge for spawning the Codex CLI and streaming JSONL run events into the UI overlay.
  - `openagents-types` — JSON Schema–backed models (plans, run events) generated with `typify`; shared by Rust and Swift integration.
  - `ui_hud` — Heads-up display elements (plan timeline, run feed) composited inside `wgpu-cube`.
- `models/` — GLTF assets for the demo (`cyborg.gltf`). `crates/wgpu-cube/build.rs` runs `npx @gltf-transform/cli` to decompress Draco meshes and embed them, so Node must be installed for full fidelity.
- `docs/` — Additional technical notes. `docs/ios-rust-wgpu.md` covers linking modes and troubleshooting for the Rust renderer.
- `scripts/` — Utility scripts (e.g., `build_rust_xcframework.sh` for distribution builds).

## Build & Verification Checklist
Follow `AGENTS.md` in the `v7` repo before handing work back:
- Rust workspace:
  - `cargo check --workspace`
  - Build touched crates explicitly (e.g., `cargo build -p tricoder-engine`, `cargo build -p wgpu-cube --target aarch64-apple-ios-sim`).
  - Ensure Node/npx is available if you edit GLTF assets so `build.rs` can regenerate embedded models.
- Xcode (Simulator default):
  - `xcodebuild -project OpenAgents/OpenAgents.xcodeproj -scheme OpenAgents -configuration Debug -sdk iphonesimulator -destination 'platform=iOS Simulator,name=iPhone 16'`
  - The pre-build script compiles the Rust static library to `target/xc-unified/<CONFIG>/librenderer_ios.a`; keep it in sync when adding crates or assets.
  - Validate the project file with `plutil -lint OpenAgents/OpenAgents.xcodeproj/project.pbxproj` if you edit Xcode settings.

For DOF or renderer experiments, hook into `crates/wgpu-cube/src/logic` (camera, pipeline state) and the HUD modules. Swift gets the renderer handle via `RustCubeRenderer`; resize and frame entrypoints already forward into Rust.

## Runtime Flow
1. Swift boots `MTKView`, installs `RustCubeRenderer`, and calls `tricoder_cube_create` (C-ABI to Rust).
2. `wgpu-cube` sets up instance/surface/device, loads the GLTF asset, and starts animating while drawing HUD overlays.
3. `RustCubeRenderer` opens a WebSocket to `codexd` (`ws://127.0.0.1:7069/ws?token=devtoken`) and forwards Codex run events into the renderer via `tricoder_engine_start_ws`.
4. Pressing “Run” issues `/start?token=devtoken&prompt=…` to `codexd`, which spawns the Codex CLI with the configured prompt and relays JSONL events.

## Pointers for Ruins Integrations
- Reuse HUD patterns from `crates/ui_hud` when designing DOF debug overlays; the HUD is renderer-agnostic and consumes the shared plan/run schemas.
- Depth-of-field pipeline research should start in `crates/wgpu-cube/src/logic` (camera setup, uniform generation) and `crates/ui_hud` for UI layering—both are minimal enough to transplant into Ruins renderer experiments.
- Keep schemas aligned: if you extend run events or plan data, update `schema/` in v7 and regenerate `openagents-types`. Mirror the changes in Ruins (see `shared/assets` and `crates/data_runtime` for schema ingestion patterns).
- `codexd` is the quickest path to replay Codex CLI sessions locally; run `cargo run -p codexd` from `~/code/v7` and point clients or tooling at `ws://127.0.0.1:7069/ws`.

## Useful References
- `~/code/v7/README.md` — High-level architecture and handshake with `openagents.com`.
- `~/code/v7/AGENTS.md` — Required build commands and Xcode validation steps.
- `~/code/v7/docs/ios-rust-wgpu.md` — Detailed Rust ↔︎ Swift linking modes and troubleshooting.
- `~/code/v7/crates/wgpu-cube/tests` — Renderer smoke tests for future regression coverage.

Keep this guide updated as the v7 repo evolves (new crates, schema changes, or shifts in the Rust/Swift interface). Update `docs/README.md` in Ruins if you move or rename this document.
