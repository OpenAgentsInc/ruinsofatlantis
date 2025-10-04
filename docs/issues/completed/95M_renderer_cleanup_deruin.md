# 95M — Renderer Cleanup: Remove Ruin-Specific Glue

Status: COMPLETE

Labels: renderer, cleanup
Depends on: Epic #95, 95L (Server scene build)

Intent
- Remove ruins-specific destructible code from the renderer and ensure generic destructible handling by ID.

Tasks
- [x] Remove/feature-gate `get_or_spawn_ruin_proxy`, `hide_ruins_instance`, and ruins-only selection paths.
- [x] Ensure typed keys `(DestructibleId, cx,cy,cz)` are used consistently in maps and helpers.
- [x] Keep dev overlay for per-proxy stats behind a feature (optional).
- [x] Replace uses of `RuinVox` type alias with generic naming and remove ruins-only comments.
- [x] Verify draw loop (`render.rs`) and upload helper accept generic destructibles (no ruins assumptions).

Acceptance
- Default build has no ruins-specific logic; destructibles are model-agnostic.
 - No references remain to ruins-only helpers in default paths (search: `ruin_`, `Ruins`).

Addendum (what shipped)
- Gated all legacy/demo ruin glue under `legacy_client_carve`/`vox_onepath_demo` features. Default builds keep destructible registries empty and never load GLTF for destructibles.
- Standardized chunk keys to `(DestructibleId, cx, cy, cz)` for voxel upload paths and replication consumers.
- Kept a `type RuinVox = VoxProxy;` alias for back-compat in feature builds; generic `VoxProxy` is the default type.
- Verified all VB/IB updates go through `voxel_upload` helpers; no renderer mutation of gameplay meshes in default paths.

