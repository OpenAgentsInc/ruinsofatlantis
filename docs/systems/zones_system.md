# Zones System

Authoring lives in `/data/zones/<slug>`. Core file: `manifest.json`.

- `ZoneManifest` declares ids, terrain, vegetation, weather, and initial TOD.
- Loader: `data_runtime::zone::load_zone_manifest`.
- Runtime: renderer reads the manifest and builds terrain/instances.

