# 95M — Renderer Cleanup: Remove Ruin-Specific Glue

Labels: renderer, cleanup
Depends on: Epic #95, 95L (Server scene build)

Intent
- Remove ruins-specific destructible code from the renderer and ensure generic destructible handling by ID.

Tasks
- [ ] Remove/feature-gate `get_or_spawn_ruin_proxy`, `hide_ruins_instance`, and ruins-only selection paths.
- [ ] Ensure typed keys `(DestructibleId, cx,cy,cz)` are used consistently in maps and helpers.
- [ ] Keep dev overlay for per-proxy stats behind a feature (optional).

Acceptance
- Default build has no ruins-specific logic; destructibles are model-agnostic.
