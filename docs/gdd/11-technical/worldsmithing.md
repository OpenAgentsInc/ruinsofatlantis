## Worldsmithing — Technical Notes

Architecture
- Single runtime; authoring is client‑side for V1. Persistence flows through the existing zone pipeline: `scene.json` → `tools/zone-bake` → `snapshot.v1/trees.json` → runtime load.
- Kind→Asset binding is data‑driven via a catalog (global with optional per‑zone overrides).

Rendering Contract
- Trees render via the textured instanced path, grouped per kind for batching.
- Renderer must bind complete groups even when assets are missing and fall back to DefaultMaterial/DefaultMesh; no WGPU validation errors are acceptable.
- Optional “ephemeral ghost” draw for the preview (tinted, no shadows, depth‑tested), submitted per frame.

Zone Policy & Caps
- Zone manifests may include a `worldsmithing` block to toggle, cap, and restrict kinds locally.
- Caps are enforced at placement time with toasts and deny when exceeded.

Data & Schemas
- Authoring JSON: `logic.spawns[]` entries with `{ id, kind: "tree.*", pos, yaw_deg }`.
- Snapshot JSON: grouped matrices per kind for instancing.
- Provide JSON Schemas and validate in CI; add a headless bake test for authoring→snapshot.

Crate Boundaries (V1)
- worldsmithing (new): input/overlay/preview/export/import/catalog; depends on client_core, ux_hud, data_runtime; thin renderer hook for ghost submission.
- data_runtime: loaders for scene snapshot and catalogs; surfaces optional `trees` to the client.
- tools/zone-bake: transform spawns→trees.json grouped by kind; update meta counts/hashes.
- render_wgpu: textured instanced pipeline, placeholders, ephemeral ghost path.

Testing
- CPU‑only tests for export/import round‑trip, bake transforms, caps logic, and policy gating. Avoid GPU/window dependencies in CI.

