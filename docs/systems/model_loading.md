# Model Loading — GLTF/GLB, Skinning, Submeshes

Scope
- Explain how our loaders ingest GLTF/GLB meshes, especially multi‑material skinned characters (e.g., Universal Base Characters), and how the model viewer exercises these paths.

Key behaviors
- Dominant skin selection:
  - Many character assets split geometry across multiple nodes that all reference the same Skin (skeleton). Our loader picks the dominant skin by vertex count and then aggregates ALL primitives from nodes that reference that skin. This fixes the common “only eyes render” artifact.
- Submesh aggregation per material:
  - Each aggregated primitive becomes a Submesh with its own baseColor texture. The viewer iterates submeshes and binds the correct texture per draw.
- Index rebasing (regular + Draco):
  - As primitives are appended, indices are rebased by the current vertex count to form a single index buffer. Draco‑compressed primitives are decoded and similarly rebased. We validate u16 range and error on overflow.
- Textures:
  - baseColor is loaded as sRGB. For now the viewer shades with baseColor only; normals/ORM are planned for a quick follow‑up.
  - Some assets export normal maps with different Y conventions; we will add a per‑texture flag when the viewer enables normals.
- Alpha/two‑sided:
  - Hair/eyelashes often use alpha. The viewer will add MASK/BLEND handling and a two‑sided toggle in a follow‑up; the loader preserves material info to enable this later.

Developer tips (viewer)
- Shrink UI text for long animation lists:
  - `cargo run -p model-viewer -- <path.gltf> --ui-scale 0.6`
- Save one‑frame PNG and exit (non‑interactive):
  - `cargo run -q -p model-viewer -- <path.gltf> --snapshot /tmp/out.png`
- Merge animations from a library into the loaded model:
  - Load a skinned model first, then click `ANIMATIONLIBRARY` in the Library pane (scans `assets/anims/**`). GLTF/GLB clips are merged into the current model by node name.
- See loader decisions in logs:
  - `RUST_LOG=info,ra_assets=info cargo run -p model-viewer -- <path>` prints lines like:
    - `skinning: selected skin index … (N verts)`
    - `append prim: verts=… idx=… material=…`

Where the code lives
- Loader: `shared/assets/src/skinning.rs`
  - Dominant skin selection, node aggregation, index rebasing, per‑primitive textures, and clip merging (`merge_gltf_animations`).
- Types: `shared/assets/src/types.rs`
  - `SkinnedMeshCPU`, `SubmeshCPU`, `TextureCPU`, animation tracks.
- Viewer: `tools/model-viewer/src/main.rs`
  - Submesh draws (one bind/draw per submesh), `--ui-scale`, `--snapshot`, library scans, and merge behavior for GLTF/GLB/FBX.

Notes
- Draco: At runtime we prefer pre‑decompressed GLTFs for static meshes. The skinned loader can decode Draco primitives but will error if indices exceed u16 after rebasing.
- LFS: Large `assets/models/**` and `assets/anims/**` are tracked via Git LFS.

