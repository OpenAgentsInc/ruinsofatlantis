# CI & DevEx Audit

CI
- Run: fmt, clippy (deny warnings), tests, Naga WGSL (already via xtask), schema check, golden packs.
- Add: `cargo deny` advisories; perf smoke budgets (frame build, mesh/collider jobs).

DevEx
- Pre‑push hooks enabled; keep fast feedback loop.
- Add `justfile`/task aliases if helpful; document ECS system tests harness.
