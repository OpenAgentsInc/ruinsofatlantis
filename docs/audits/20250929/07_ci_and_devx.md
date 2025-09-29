# CI & DevEx Audit

Current
- `xtask ci` runs: fmt, clippy (`-D warnings`), tests, and schema-check. This is a solid baseline.
- Policies captured in `AGENTS.md` are strong (no interactive runs, Cargo.toml hygiene, build green requirement).

Gaps
- No shader (WGSL) validation.
- No dependency policy enforcement (`cargo deny`).
- No perf smoke budgets in automation.
- No toolchain pin (`rust-toolchain.toml`) to stabilize CI runs across contributors.

Recommendations
1) Strengthen `xtask ci`
- Add WGSL validation (Naga) across all `*.wgsl` files.
- Add `cargo deny` (advisories, licenses, bans) with a baseline config.
- Add headless renderer CPU hashing tests and sim-harness scenario check.

2) Toolchain & Cache
- Pin stable toolchain via `rust-toolchain.toml` and document update cadence.
- Set up sccache or build cache via CI runners where appropriate.

3) Perf Smoke (Nightly/Optional)
- Run a reduced scenario and CPU-only renderer buffer build; record times and assert envelopes vs budgets documented in `AGENTS.md`.

4) Pre-commit Hooks (Optional)
- Provide `.githooks` with fmt/clippy/tests subset and instructions to enable locally; keep CI as the enforcement point.

