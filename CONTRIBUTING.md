# Contributing

Thanks for helping build Ruins of Atlantis.

- Branch from `main` using `area/summary` (e.g., `gfx/fix-bottom-ghost`).
- Keep changes focused. One PR per logical change.
- Always run `cargo xtask ci` locally before pushing.
- Update GDD and `docs/gdd/**` when design‑level behavior changes.
- Include screenshots for rendering/UI changes; perf note if GPU cost changed ≥0.5 ms.

## PR checklist
- [ ] `cargo xtask ci` passes (fmt, clippy -D warnings, tests, schema)
- [ ] Updated docs (GDD, docs/gdd)
- [ ] Added/updated tests (unit + golden if packs changed)
- [ ] SRD attribution in `NOTICE` if needed

## Commit/PR style
- Commits: `area: imperative summary`
- PRs: `area: imperative summary` + details (what/why, before/after, screenshots).

## Ownership
See `CODEOWNERS` for auto‑requested reviewers per path.
