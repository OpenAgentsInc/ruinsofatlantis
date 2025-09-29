# Frame Graph (Prototype)

This prototype uses a minimal static frame-graph to document and validate pass I/O.

Resources
- SceneColor: offscreen HDR color target
- SceneRead: readback copy of SceneColor for post passes that sample color
- Depth: depth texture (linearized pyramid is derived and read-only in passes)

Passes
- sky: writes SceneColor
- main: reads Depth, writes SceneColor
- blit_scene_to_read: reads SceneColor, writes SceneRead (only when not direct-present)
- ssr: reads Depth + SceneRead, writes SceneColor
- ssgi: reads Depth + SceneRead, writes SceneColor
- post_ao: reads Depth, writes SceneColor
- bloom: reads SceneRead, writes SceneColor

Invariants
- A pass must not sample from a resource it writes in the same frame.
- Depth is read-only.

Implementation
- See `crates/render_wgpu/src/gfx/renderer/graph.rs` for the definition and a simple per-frame validation.
- Render order stays explicit in `render.rs`; the graph is used to catch mistakes and to document pass I/O.
