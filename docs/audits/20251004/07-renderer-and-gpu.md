# Renderer & GPU — 2025-10-04

Context
- `render_impl` mixes input/controller updates, simple AI, replication buffer drains, and draw submission (crates/render_wgpu/src/gfx/renderer/render.rs:12+).
- Hot-loop allocations: numerous `.clone()` on large CPU buffers and state in renderer and server upload paths (evidence/hotloop-allocs.txt).
- Surface error handling present in some paths; ensure unified handling in the main render loop.

Findings
- F-ARCH-002: Renderer hosts gameplay/AI/input — move to `client_core` systems (P1 High).
- F-RENDER-004: Excessive cloning in hot paths — apply caching and references; persist buffers where possible (P2 Med).
- F-RENDER-012: Unsafe lifetime transmute for `wgpu::Surface`; ensure narrow scope and safe reconfigure on lost/outdated (crates/render_wgpu/src/gfx/renderer/init.rs:84,91) (P3 Low).

Recommendations
- Narrow renderer to upload/draw only; consume ECS/replication outputs. Move controller/AI into `client_core`.
- Cache bind groups, textures, and CPU buffers; avoid per-frame re-creation; prefer staging buffers with persistent allocations.
- Ensure surface lost/outdated paths reconfigure cleanly and drop/recreate dependent resources as needed.
- Consider a light frame-graph to encode read/write dependencies and avoid hazards.

