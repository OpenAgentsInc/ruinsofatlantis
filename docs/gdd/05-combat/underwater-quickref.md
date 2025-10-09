- sim-core: deterministic rules engine (fixed timestep, e.g., 50 ms). Holds entities, stats, cooldowns, effects, threat, concentration, and an event bus. No rendering.
- sim-data: SRD-derived data (JSON/TOML) for classes, spells, conditions, monsters. Versioned IDs and provenance.
- sim-policies: tactical policies (boss AIs, player rotas/priority lists, movement heuristics). Pluggable strategies.
- sim-harness: CLI runner for scenarios, sweeps, and metrics export (CSV/JSON).
- sim-viz (optional): minimal wgpu/winit debug renderer (orthographic), or TUI for timelines/logs.

Determinism & timestep
- Fixed-timestep loop (e.g., 20 Hz/50 ms) with discrete-event scheduling for casts, cooldowns, DoTs/HoTs.
- Seeded RNG per run and per-actor streams; all random draws (hit, crit, save) come from these streams.
- Net-latency model: per-actor input delay and server tick alignment for realistic cast/queue timing.

Scenario format
- YAML/JSON: map, actors (class/build), gear tier, boss type, initial positions, policies, win/lose conditions, and metrics to collect.
- Example: boss: aboleth, underwater: true, depth: shallow, party: [fighter_tank, cleric_heal, wizard_ctrl, rogue_dps, monk_dps, ranger_dps].
