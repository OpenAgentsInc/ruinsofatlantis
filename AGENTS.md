# Repository Guidelines

## Project Structure & Module Organization
- Root: `README.md`, `GDD.md` (design), `LICENSE`, `NOTICE`.
- Rust workspace (planned): `Cargo.toml` at root; crates under `server/`, `client/`, `shared/`, `tools/` (not final).
- Assets: `assets/` (art/audio), `data/` (JSON/CSV), `docs/` (design notes, diagrams).
- Tests: unit tests within each crate’s `src/`; integration tests in top‑level `tests/`.

## Build, Test, and Development Commands
- Build all crates: `cargo build`
- Run formatter and lints: `cargo fmt` and `cargo clippy -- -D warnings`
- Run all tests: `cargo test`
- Run server (when present): `cargo run -p server`
- Example with logs: `RUST_LOG=info cargo run -p server`

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
- PRs must: describe changes, include screenshots for UI, update docs (`README.md`/`GDD.md`), update `NOTICE` if SRD content is used, and pass build/fmt/clippy/tests.

## SRD, Licensing, and Attribution
- SRD 5.2.1 content is CC‑BY‑4.0. Include the exact attribution in `NOTICE` and keep GDD’s SRD section accurate.
- This project is D&D‑inspired/“5E compatible,” but unofficial; do not imply endorsement; avoid Wizards’ trademarks/logos.
- Keep `LICENSE` (Apache‑2.0) intact; add third‑party notices under `NOTICE`.

## Security & Configuration Tips
- Never commit secrets. Use env vars and a `.env.example`; ignore real `.env` files.
- Prefer local config under `config/` with sample defaults; document required vars in `README.md`.
