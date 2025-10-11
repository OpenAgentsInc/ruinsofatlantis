- RON/TOML/YAML: nicer comments than JSON, but weaker editor/CI ecosystem and typically slower/looser parsers; if comments are required, RON/YAML is acceptable for authoring, still compile to binary for runtime.
- SQLite pack: useful for patching/queries, but overkill early; a binary blob is simpler and faster to start.

Performance notes (what we’ll do)
- Precompute AoE samplers, save DC resolvers, mitigation‑order tables, condition IDs, and damage‑type masks.
- Intern strings and use numeric IDs at runtime.
- Avoid dynamic dispatch in the hot path: small enum opcodes dispatched via a jump table.
- Keep cold data compressed: ship one spellpack per version; memory‑map and build in‑memory indices on first use.

Safety & SRD fidelity
- A data‑driven spell system makes it straightforward to verify SRD mechanics (save DCs, components, Concentration, resistance/vulnerability order, THP non‑stacking, roll‑once AoE damage) and to document any MMO‑layer deviations; this is easier to audit than logic scattered across code.

Concrete recommendation
- Use JSON for authoring (or RON if comments are required).
- Add `spell_schema.json`, CI validation, and a build step that emits `spellpack.bin`.
- Support hot‑reload JSON in development, load only binary in release.
- Bake in content hashes and versioning; fail fast if client/server spellpack hashes mismatch.
 

## Environment: Sky & Weather

**Design Intent.** A physically‑plausible, configurable sky that animates day/night, drives sun light and ambient skylight, and supports per‑zone weather variation—consistent with RoA’s “in‑world, no toggles” philosophy.

**Player Experience.**

* Sun and sky progress naturally through the day; dawn/dusk tint the world.
* Overcast, haze, and fog vary by zone (swampy lowlands vs. coastal cliffs).
* Lighting changes are readable and influence visibility and mood.

**Scope (Phase 1).**

* Analytic clear‑sky model (Hosek–Wilkie) evaluated per pixel.
* Sun position from game time (day‑fraction) with optional geographic driver.
* Directional sunlight + **SH‑L2** ambient skylight for fill.
* Distance/height‑based fog. Optional simple tonemapper (Reinhard / ACES fit).
* Per‑zone weather overrides: turbidity, fog density/color, ambient tint, exposure.
* Tooling hooks in `tools/model-viewer`.

**Data & Authoring.**

* `data/environment/defaults.json` (global), `data/environment/zones.json` (overrides).
* Runtime controls: pause/scrub time, rate scale.
* Debug: show azimuth/elevation; sliders for turbidity/fog/exposure.

**Runtime Behavior.**

* **Renderer order:** sky → shadows → opaque → transparent/FX → UI.
* **Lighting:** `sun_dir_ws`, `sun_illuminance`, `sh9_ambient` in `Globals` UBO.
* **Zones:** entering a WeatherZone blends to its profile over 0.5–2.0s.

**Integration Points.**

* Terrain/biomes shading uses directional + SH ambient.
* Minimap shows weather glyph; HUD clock displays zone time.
* Sim/Events may trigger storms later (Phase 2).

**Performance Targets.**

* Sky pass ≤0.2 ms; SH projection ≤0.1 ms/frame amortized; single shadow map in Phase 1.

**Future Work.**

* Volumetric clouds and aerial perspective; precipitation; moon/stars; cascaded shadows.

See: `docs/gdd/12-environment/sky-weather.md` for the authoritative system doc.

---

## World: Terrain & Biomes

**Design Intent.** Fast, attractive terrain that varies by biome and is **procedurally generated once, then baked** into persistent zone snapshots. Phase 1 focuses on a Woodland baseline (rolling hills, dense grass, scattered trees).

**Player Experience.**

* Natural rolling hills; grass thick near the player; trees spaced believably.
* Layout is stable across sessions/players (persistent zone), not re‑rolled.

**Scope (Phase 1).**

* Heightfield generation: **OpenSimplex2 fBm + domain warping**.
* Chunked mesh (e.g., 64×64 verts) with simple distance LOD and skirts.
* **Triplanar** material with slope/height blending (grass/dirt/rock).
* Vegetation:

  * **Trees** from GLB prototypes placed via **Poisson‑disk** (baked, instanced).
  * **Grass** as GPU‑instanced cards with density masks per chunk (baked).
* Bake tool writes `snapshot.terrain.bin`, `snapshot.instances.bin`, masks, meta.

See: `docs/gdd/12-environment/terrain-biomes.md` for the authoritative system doc.
---

## Tools: Worldsmithing (Authoring Pipeline)

- In‑world authoring persists through data → bake → snapshot.
- Authoring JSON (`scene.json`) lists `logic.spawns[]` for kinds like `tree.default`.
- Bake writes grouped matrices per kind to `snapshot.v1/trees.json` for instanced rendering.
- Renderer must bind complete groups even when assets are missing; use DefaultMaterial/DefaultMesh fallbacks.
- See `docs/gdd/11-technical/worldsmithing.md` for technical details and crate boundaries.
