# Build, CI/CD — 2025-10-04

Status
- `xtask ci` orchestrates fmt, clippy, WGSL validation, tests, schema checks (grep in xtask, README usage).
- `cargo fmt --all --check` reports diffs (evidence/fmt-diff.txt).
- `cargo clippy --all-targets --all-features` completed successfully (evidence/clippy.txt tail shows Finished).
- `cargo test --all --no-run` failed with unresolved imports in test modules (evidence/warnings.txt), indicating missing dev-deps.

Findings
- F-CI-005: Test compilation failures due to missing dev-deps in crates referencing `core_*` in tests (P1 Med).
- F-CI-013: Add `cargo deny` to CI and fail on advisories; xtask already hints when not installed (P3 Low).

Recommendations
- Fix test module imports by adding required dev-deps via `cargo add -p <crate> --dev core_units core_materials` (and others as needed).
- Enforce fmt in CI and apply formatting changes indicated in evidence/fmt-diff.txt.
- Integrate `cargo deny` into `xtask ci` and GitHub Actions if present.

