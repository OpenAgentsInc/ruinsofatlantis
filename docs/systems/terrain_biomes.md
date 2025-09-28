# Terrain & Biomes (Phase 1)

Deterministic CPU heightmap: size/extent/seed from `ZoneManifest`.

- Normals computed on load; mesh uploaded once.
- Tree scatter: slope/height filtered, seeded.
- Optional baked snapshots under `packs/zones/<slug>/snapshot.v1/*`.

