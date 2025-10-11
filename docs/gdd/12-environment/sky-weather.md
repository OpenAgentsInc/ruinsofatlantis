# Sky & Weather

Runtime sky uses Hosek–Wilkie; ambient is SH‑L2. `SkyStateCPU` owns time‑of‑day and recompute.

- Day fraction `[0..1]` → sun direction (vertical arc + azimuth offset).
- Night: radiance and ambient are darkened with a steep ramp and small floor.
- Authors set initial TOD via zone manifest (`start_time_frac`, `start_paused`).

