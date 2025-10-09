- Artifacts: event logs (NDJSON), timelines, replay seeds.

Visualization (optional)
- Headless CSV/JSON by default. Debug modes: TUI (timelines, threat meter) and simple wgpu orthographic render (positions, AoEs, cast bars).
- Replays: load event log + seed to step or scrub.

CLI (proposed)
- Single run: `cargo run -p sim-harness -- --scenario scenarios/aboleth.yaml --seed 42 --tick 50ms --log results/run.ndjson`
- Monte Carlo: `... --trials 1000 --vary policy=tank_a,tank_b --out results/aboleth.csv`
- PvP skirmish: `... --mode pvp --team-a scenarios/team_a.yaml --team-b scenarios/team_b.yaml`

Next steps
- Define sim-core state and event types; draft Aboleth encounter from this GDD.
- Implement priority policy for the six-player party; add baseline boss AI.
- Add metrics collectors and CSV exporter; wire seeds and determinism tests.

## Zones & Cosmology

Pulled from SRD 5.2.1 cosmology. We keep the canonical plane names (Material, Feywild, Shadowfell, Inner Planes, Outer Planes, Astral, Ethereal) and describe how they manifest in an Atlantis‑ruins, oceanic MMO world.

### Material Plane
- Primary game world of shattered continents, sunken cities, and Atlantean ruins.
- Both surface archipelagos and deep‑sea environments are fully explorable.
- Baseline adventuring setting for survival, exploration, and faction conflict.

### Feywild
- Accessed via coral gates, shimmering lagoons, or enchanted whirlpools.
