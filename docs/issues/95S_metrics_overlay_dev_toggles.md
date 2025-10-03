# 95S — Metrics Overlay & Dev Toggles

Labels: developer-experience, observability
Depends on: Epic #95, 95J (Jobs), 95E (Server systems)

Intent
- Surface per-system timings and backlogs in an overlay; add toggles to assist tuning.

Tasks
- [ ] Server: expose counters/timings for mesh/collider jobs, carve counts.
- [ ] Client: overlay panel reads replicated metrics or uses in-proc hooks for local runs.

Acceptance
- Hotkey toggles overlay; warns when budgets exceeded.
