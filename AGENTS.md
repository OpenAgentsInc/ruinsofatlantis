# Repository Guidelines

## Project Structure & Module Organization
- Root: `README.md`, `GDD.md` (design), `LICENSE`, `NOTICE`.
- Workspace: multiple crates under `crates/` and `shared/`; root remains the app shell and bins.
- Assets: `assets/` (art/audio), `data/` (JSON/CSV), `docs/` (design notes, diagrams).
- Tests: unit tests within each crate’s `src/`; integration tests in top‑level `tests/`.

### Workspace Crates (current)
- `crates/render_wgpu` — Renderer crate. Hosts the full renderer under `src/gfx/**` (camera, pipelines, shaders, UI, scene build, terrain, sky, etc.). Root `src/gfx/mod.rs` is a thin re‑export of this crate.
- `crates/platform_winit` — Platform/window/input loop (winit 0.30) that drives the renderer. Root app calls `platform_winit::run()`.
- `crates/sim_core` — Rules/combat FSM and headless sim engine (`rules/*`, `combat/*`, `sim/*`).
- `crates/data_runtime` — SRD‑aligned data schemas + loaders (replaces `src/core/data`).
- `crates/ux_hud` — HUD logic/toggles (`HudModel`), separated from renderer draw code.
- `crates/ecs_core` — Minimal ECS (entities, transforms, render kinds) used by scene assembly.
- `crates/client_core` — Input state and third‑person controller used by the app/renderer.
- `crates/server_core` — In‑process NPC state + simple AI/collision avoidance used by the demo.
- `shared/assets` — Asset loaders/types (GLTF/Draco helpers) used by tools and the renderer (import as `ra_assets`).

### Time‑of‑Day & Weather
- Zone authors can control initial time‑of‑day via `data/zones/<slug>/manifest.json`:
  - `start_time_frac` (0.5 = noon; ~0.0/1.0 = midnight)
  - `start_paused` (freeze TOD; Space toggles in the client)
  - `start_time_scale` (TOD playback rate when not paused)
- The renderer applies these to `SkyStateCPU` on startup; the sky/ambient is deliberately
  darkened at night so emissive VFX read strongly.

### What lives in `src/` and why
- `src/main.rs` — App entry; initializes logging and calls `platform_winit::run()`.
- `src/gfx/mod.rs` — Thin re‑export of `render_wgpu::gfx` for app code.
- Bins moved to `tools/` crates to keep the app shell slim. The old `wizard_viewer` and other probes now live under `tools/`.

### Tools (moved out of `src/bin`)
- `tools/model-viewer` — Standalone wgpu GLTF/GLB viewer (uses `shared/assets`).
- `tools/zone-bake` — Bakes terrain + trees snapshot for a Zone slug. Usage: `cargo run -p zone-bake -- <slug>`.
- `tools/gltf-decompress` — One-time Draco decompressor for GLTFs. Usage: `cargo run -p gltf-decompress -- <in> <out>`.
- `tools/image-probe` — Simple image stats probe. Usage: `cargo run -p image-probe -- <png>`.
- `tools/bevy-probe` — Bevy-based material/texture extractor for the wizard asset.

Note: The old `src/core/` facade and `src/assets/` facade were removed. Crates should import `data_runtime`, `sim_core`, and `ra_assets` directly.

Guidance: new reusable systems (renderer modules, platform, data, sim, HUD logic, ECS, client/server glue, tools libraries) should live in dedicated crates. App glue, small bins, and short‑lived prototypes can remain in `src/` until they harden, at which point prefer moving them under `crates/` or `tools/`.

### Code Organization (renderer)
- Rendering lives under `crates/render_wgpu/src/gfx/` split by responsibility (camera, types, mesh, pipeline, shaders, ui, terrain, sky, temporal, framegraph helpers). The root `src/gfx/mod.rs` re‑exports this for compatibility.
- Keep modules cohesive and well‑documented; prefer adding focused sub‑modules as systems grow (scene graph, streaming, UI, net, ECS, etc.).

### Documentation & Comments
- All new modules must start with a brief docblock explaining scope and how to extend it.
- Add inline comments for non‑obvious math, layout decisions, and API quirks (e.g., WGSL/std140 padding, wgpu limits).
- Prefer doc comments (`///`) on public types/functions so `cargo doc` is useful.
- Do not add meta comments like "(unused helper removed)" or "(logs removed)". If code is unused, delete it; keep comments focused on behavior and intent, not change notes.
- When removing logging or debug prints, do not leave placeholder comments (e.g., "no info log" or similar). Remove quietly unless there’s a behavioral reason to document.

IMPORTANT: Keep `src/README.md` current
- Whenever you add, move, or significantly change files under `src/`, immediately update `src/README.md` to reflect the real file/folder hierarchy and module responsibilities.
- Document new pipelines, shaders, UI overlays, systems, and data flows so future contributors can navigate quickly.

## Game Design Document (GDD)
- Canonical design source: `GDD.md` at repo root.
- Keep these sections current: Philosophy, Game Mechanics, SRD Usage and Attribution, SRD Scope & Implementation, and any Design Differences from SRD.
- Before gameplay/rules changes, update `GDD.md` in the same PR; explain rationale and SRD impact.
- SRD usage: maintain exact attribution in `NOTICE`; document any terminology deviations (e.g., using “race”).

## Golden Rules
- Never run interactive apps in CI or automation. Use `cargo xtask ci`.
- Never hand‑edit `Cargo.toml`; use `cargo add/rm/upgrade`.
- Never commit uncompiled packs under `/packs` without running `cargo xtask build-packs`.
- Never add binary assets outside LFS. Track large binaries via git‑lfs.
- Always keep the repo compiling with tests green before handoff.
- Always update GDD and `docs/systems/*.md` when design behavior changes.

## Input & Keybinding Policy
- Do not bind default gameplay or debug actions to function keys (F1–F12). Browsers and OSes often reserve these; they are unreliable on the web.
- Prefer letters/digits and simple modifiers that work on desktop and in browsers. Current bindings:
  - `P` toggles the perf overlay (was F1)
  - `O` triggers a 5s automated orbit for screenshots (was F5)
  - `H` toggles the HUD; `Space`, `[`/`]`, and `-`/`=` control time‑of‑day
- If you add new inputs, choose keys that won’t clash with common browser shortcuts (e.g., avoid `Ctrl+L`, `Cmd+F`, etc.). Document changes in `src/README.md`.

## Ownership Map
- `crates/render_wgpu/**` → Graphics owners
- `crates/sim_core/**` → Gameplay/systems owners
- `crates/data_runtime/**` → Data/Schema owners
- `crates/ux_hud/**` → UI/HUD owners
- `/data/**` & `/packs/**` → Content pipeline owners
- `/tools/**` → Tools team
- `/docs/**` and `GDD.md` → Design owners

## Branch & PR Policy
- Branch name: `area/short-summary` (e.g., `gfx/fix-bottom-ghost`).
- PR title: `area: imperative summary`.
- Must include screenshots for UI, perf note if GPU cost changed ≥0.5 ms, and schema diffs if data changed.

## Build, Test, and Development Commands
- Prerequisites: Install Rust via `rustup` (stable toolchain). If the edition is unsupported, run `rustup update`.
- Run the app (root crate): `cargo run` (default-run is `ruinsofatlantis`)
- Build (debug/release): `cargo build` / `cargo build --release`
- Run with logs: `RUST_LOG=info cargo run`
- Tests: `cargo test`
- Format/lint: `cargo fmt` and `cargo clippy -- -D warnings`
- Optional dev loop: `cargo install cargo-watch` then `cargo watch -x run`

### Golden Commands
```
# build + lint + test + schema + pack
cargo xtask ci

# when editing spells or zones
cargo xtask build-packs
cargo xtask bake-zone --slug wizard_woods
```

### Git Hooks & Pre‑Push Policy
- Enable repo hooks locally so pushes run the same checks as CI:
  - `./scripts/setup-git-hooks.sh` (preferred), or `git config core.hooksPath .githooks`
- The Pre‑Push hook (`.githooks/pre-push`) runs the full workspace pipeline:
  - `cargo xtask ci` (fmt + clippy ‑D warnings + WGSL validation via Naga + cargo‑deny if installed + tests + schema checks)
  - It falls back to `cargo run -p xtask -- ci` if `cargo xtask` isn’t installed.
- Skipping (rare, last resort): set `RA_SKIP_HOOKS=1` for a single push (e.g., to push a CI fix). Do not use this to bypass broken code.
- Policy: Do not push if the pre‑push hook or `cargo xtask ci` fails locally.

NOTE FOR AGENTS
- Do NOT run the interactive application during automation (e.g., avoid invoking `cargo run`) unless the user specifically directs you to. The user will run the app locally. Limit yourself to building, testing, linting, and file operations unless explicitly asked otherwise.

## Assets & GLTF
- Place models under `assets/models/` (e.g., `assets/models/wizard.gltf`).
- GLTF loader uses `gltf` crate with the `import` feature, so external buffers/images resolve via relative paths. Keep referenced files next to the `.gltf` or adjust URIs accordingly.
- Current prototype loads two meshes (`wizard.gltf`, `ruins.gltf`) and draws them via instancing.
- If a model is Draco-compressed (e.g., `ruins.gltf`), prepare a decompressed copy once:
  - `cargo run -p gltf-decompress -- assets/models/ruins.gltf assets/models/ruins.decompressed.gltf`
  - The runtime prefers `*.decompressed.gltf` if present.
- When adding dependencies for loaders or formats, use `cargo add` (never hand‑edit `Cargo.toml`).
- Draco compression: The runtime does NOT attempt decompression. If a model declares `KHR_draco_mesh_compression`, run our one-time helper:
  - `cargo run -p gltf-decompress -- assets/models/foo.gltf assets/models/foo.decompressed.gltf`
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

### Testing Matrix
- Unit tests (per crate): math/helpers, effect opcodes, mitigation order, THP rules, ECS utilities.
- Golden tests (packs): fixed seeds → golden outputs; compare bytes.
- Sim harness tests: deterministic 100‑cast scenarios per MVP spell.
- Headless renderer tests: build vertex buffers and culling lists on CPU; hash them.
- Perf smoke (nightly): measure frame build time and assert budgets.

### Determinism Rules
- All RNG is seeded; seeds logged in artifacts.
- No system may read wall‑clock in hot paths; time is injected.
- Prefer integer math in sim logic when feasible.

## Renderer Hygiene
- Run WGSL validation (Naga) as part of `cargo xtask ci`.
- Common WGSL headers under `gfx/shaders/common/*`.
- No global Y‑flips; flips are local per sampling path.
- Post samplers must be `ClampToEdge`.
- Framegraph: never sample from a texture written this frame unless copied to a read target.

## Performance Budgets
| Subsystem        | Budget (ms @1080p mid‑GPU) | Notes                 |
| ---------------- | --------------------------- | --------------------- |
| UI/HUD           | ≤ 0.5                       | batched ≤ 30 draws    |
| Sky + fog        | ≤ 0.3                       | HW sky + SH ambient   |
| Terrain + grass  | ≤ 5.0                       | default radius        |
| Shadows (single) | ≤ 1.0                       | CSM later             |
| Particles        | ≤ 1.0                       | CPU→GPU ring          |
| Sim tick         | ≤ 1.0                       | 20 Hz fixed           |

Rule: If a PR changes GPU cost by ≥0.5 ms, include a perf note + capture.

## Data Authoring
- Schemas live in `crates/data_runtime/schemas/` (JSON Schema or serde‑validated RON).
- `cargo xtask schema-check` validates all `/data/**` against models/schemas.
- Version every format and content‑hash packs.
- Schema migrations must ship a migrator (`xtask migrate ...`) and a release note.

### Data Change Checklist
- [ ] Updated `/data/**` and validated (`cargo xtask schema-check`)
- [ ] Rebuilt packs (`cargo xtask build-packs`) and committed `/packs/**`
- [ ] Updated docs in `/docs/systems/*.md` or `GDD.md`
- [ ] Added/updated tests (unit + golden)
- [ ] Updated `NOTICE` for SRD attribution when needed

### Release Artifacts
- Tagging a release auto‑attaches:
  - `packs/spellpack.v1.bin`
  - `packs/zones/<slug>/snapshot.v1/*`
  - CHANGELOG excerpt generated from PR titles

## Issue Labels & Boards
- Labels: `area:*`, `type:*`, `prio:P0..P3`, `perf`, `determinism`, `schema-change`, `docs-needed`.
- Kanban: Backlog → Ready → In Progress → Review → Done.

## Commit & Pull Request Guidelines
- Commit style: `<area>: <imperative summary>` (e.g., `server: add login flow`).
- Include what/why in body; link issues (e.g., `#123`).
- PRs must: describe changes, include screenshots for UI, update design docs (`GDD.md`), update `NOTICE` when SRD content is added, and pass build/fmt/clippy/tests.
- PR text hygiene: Use real newlines in PR descriptions and check rendering. Do not paste literal `\n` sequences. When using `gh` CLI, pass a proper multiline body (e.g., with a heredoc or `$'...'` quoting). Preview the PR body before submitting.

## SRD, Licensing, and Attribution
- SRD 5.2.1 content is CC‑BY‑4.0. Include the exact attribution in `NOTICE` and keep GDD’s SRD section accurate.
- This project is D&D‑inspired/“5E compatible,” but unofficial; do not imply endorsement; avoid Wizards’ trademarks/logos.
- Keep `LICENSE` (Apache‑2.0) intact; add third‑party notices under `NOTICE`.

## Security & Configuration Tips
- Never commit secrets. Use env vars and a `.env.example`; ignore real `.env` files.
- Prefer local config under `config/` with sample defaults; document required vars in `README.md`.
