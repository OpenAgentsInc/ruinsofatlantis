# Foliage: Baked Trees and Texturing

Scope
- How Campaign Builder trees are authored, baked, and rendered.
- Asset locations and kind→filename mapping (Quaternius kit).
- Current renderer behavior and what’s next for full textures.

Authoring → Bake → Runtime
- Author trees in `data/zones/campaign_builder/scene.json` under `logic.spawns[]` with `kind` starting with `tree.`.
- Bake snapshot: `cargo run -p zone-bake -- campaign_builder` writes `packs/zones/campaign_builder/snapshot.v1/trees.json` with:
  - `models`: column‑major 4x4 transforms (yaw+translation; Y snapped to terrain at runtime).
  - `by_kind`: optional map of `kind_slug` → transforms used for per‑kind draws.
- Runtime loader prefers workspace `data/zones/<slug>/snapshot.v1/trees.json`, falls back to `packs/...`.

Direct boot into Campaign Builder
- `ROA_ZONE=campaign_builder cargo run` to skip the picker.

Kinds and asset mapping
- Use `kind` values like `tree.quaternius.Birch_1` or families like `tree.pine`.
- Mapping lives in `crates/render_wgpu/src/gfx/foliage.rs: path_for_kind()`:
  - `quaternius.<Model>` → `assets/trees/quaternius/glTF/<Model>.gltf` (e.g., `Birch_1.gltf`).
  - Families (`pine`, `giantpine`, `tallthick`, `twistedtree`, `deadtree`, `cherryblossom`) map to a representative `.gltf`.
- Assets are vendored under `assets/trees/quaternius/glTF/` (GLTF + referenced textures). No remote paths.

Renderer behavior (V1)
- Trees are drawn via instancing. Precedence:
  1) Baked snapshot (by_kind if present) → used as‑is.
  2) Procedural scatter when no snapshot exists.
- If a baked snapshot exists, manifest `vegetation.tree_count=0` does NOT disable trees.
- Current default instanced path uses a flat albedo tint (greenish). That’s why all trees appeared green.

Textured instancing status
- A textured instancing path is scaffolded (UV vertex layout, per‑kind material bind groups).
- Remaining work (tracked in code TODOs):
  - Align WGSL bind group indices for the material texture/sampler with the pipeline layout.
  - Finalize per‑kind material bind‑group caching (avoid re‑import per frame).
- Once enabled, the renderer will sample baseColor textures from the Quaternius GLTFs and trees will render fully textured.

Troubleshooting
- “Failed to load GLTF tree mesh for kind … falling back to cube” → ensure the mapped file exists under `assets/trees/quaternius/glTF/` with exact casing, or set `RA_TREE_PATH` to an absolute GLB/GLTF while testing.
- “baked trees snapshot appears collapsed” → bake produced zero/near‑identical transforms; re‑export from the Builder and re‑bake.

