Zone System — Runtime Integration (v0)

Goals
- Renderer does not branch on demo/gameplay nouns. It draws static world from a Zone snapshot (if present) and draws actors from replication.
- Server (demo in this repo) decides what to spawn. Switching content means choosing a different Zone.

Data layout
- Baked snapshots live under `packs/zones/<slug>/snapshot.v1/` and may contain:
  - `instances.bin`, `clusters.bin` — static instancing and culling data
  - `colliders.bin`, `colliders_index.bin` — physics colliders
  - `meta.json` — optional metadata (bounds, display name)

Client
- Loader: `client_core::zone_client::ZonePresentation::load(slug)` reads the snapshot root.
- Upload: `render_wgpu::gfx::zone_batches::upload_zone_batches(&Renderer, &ZonePresentation)` returns a `GpuZoneBatches` handle.
- Renderer API: `Renderer::set_zone_batches(Some(GpuZoneBatches))` attaches the zone. When present, renderer skips legacy ad‑hoc scene content and draws only Zone static + replicated actors.

Server (demo)
- The platform demo server now checks the selected zone and only spawns encounter actors (rings/wizards/boss) when not running the minimal controller demo.

Selecting a zone
- Native: `ROA_ZONE=<slug> cargo run -p platform_winit`
- Web: append `?zone=<slug>` to the URL (e.g., `/play?zone=cc_demo`).
- Back‑compat: `RA_ZONE` is still accepted if set, but `ROA_ZONE` is the canonical env var going forward.

Future work
- Add `.roazone` authoring files and a bake tool emitting `snapshot.v1/*` blobs.
- Expand `ZonePresentation` and `GpuZoneBatches` to include real static instancing.
- Move the demo server’s zone–>spawn logic into `server_core::zones` when the authoring data is ready.

