# 95Q — Remove Legacy Client Carve & Demo Paths

Labels: cleanup
Depends on: Epic #95, 95F (Renderer upload), 95I (Replication)

Intent
- Remove legacy client carve/collider/mesh/debris mutation code and demo paths once server-authoritative path is proven.

Tasks
- [ ] Delete gated carve/collider logic and demo grid code; keep `vox_onepath_demo` only for tooling if wanted.
- [ ] Update docs to reflect server-authoritative flow.

Acceptance
- No client code paths mutate voxels/colliders; simplified renderer.
