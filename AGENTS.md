# Repository Guidelines

## Project Structure & Module Organization
- Root: `README.md`, `GDD.md` (design), `LICENSE`, `NOTICE`.
- Rust crate (current): single binary at root with `Cargo.toml` and `src/main.rs`.
- Future workspace (planned): crates under `server/`, `client/`, `shared/`, `tools/`.
- Assets: `assets/` (art/audio), `data/` (JSON/CSV), `docs/` (design notes, diagrams).
- Tests: unit tests within each crate’s `src/`; integration tests in top‑level `tests/`.

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

## Dependency Management
- Never add dependencies by hand in `Cargo.toml`.
- Always use Cargo’s tooling so versions/resolution are correct:
  - Install: `cargo install cargo-edit`
  - Add deps: `cargo add <crate> [<crate>...]` (e.g., `cargo add winit wgpu log env_logger anyhow`)
  - Remove deps: `cargo rm <crate>`
  - Upgrade: `cargo upgrade` (review diffs before committing)

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
