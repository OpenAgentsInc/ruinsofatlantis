# Controls and Input Profiles

This document describes the client controls, input profiles, and configuration for the Neverwinter‑style action combat scheme.

Overview
- Default profile: Action/Reticle — persistent mouselook with a center reticle.
- Classic fallback: Classic Cursor — cursor visible by default; holding RMB engages temporary mouselook; releasing RMB restores the cursor.
- ALT behavior is configurable (toggle or hold‑to‑hold). In hold mode, pressing ALT releases the cursor and on release returns to mouselook.

Key bindings
- Mouse: LMB/RMB — At‑Will abilities (emits `InputCommand::AtWillLMB/AtWillRMB`)
- Keyboard: Q/E/R — Encounter abilities; Shift — Dodge/Guard; Tab — Class mechanic
- ALT — Cursor toggle (default) or hold (configurable)
- Scroll — Camera zoom; WASD — movement; optional legacy orbit with RMB drag

Config file (optional)
- Path: `data/config/input_camera.toml`
- Keys:
  - `sensitivity_deg_per_count` (float)
  - `invert_y` (bool)
  - `min_pitch_deg` / `max_pitch_deg` (float degrees)
  - `alt_hold` (bool): true = hold‑to‑hold; false = toggle
  - `profile` (string): `ActionCombat` or `ClassicCursor`

Example
```
sensitivity_deg_per_count = 0.12
invert_y = false
min_pitch_deg = -75
max_pitch_deg = 75
alt_hold = true
profile = "ActionCombat"
```

Environment overrides
- `MOUSE_SENS_DEG`, `INVERT_Y`, `MIN_PITCH_DEG`, `MAX_PITCH_DEG`, `ALT_HOLD`, `INPUT_PROFILE`

Logging
- Controller mode transitions are logged at `info` with target `controls`.
- Action bindings enqueue events are logged at `info` with a `debug` snapshot of pressed inputs.
- Set `RUST_LOG=info,client_core=info` (or configure `tracing` subscriber) to observe transitions; use `debug` for detailed snapshots.

Notes
- The client only emits `InputCommand` events. Server remains authoritative for gameplay.
- On platforms where pointer‑lock is denied (browser/OS), the client falls back to cursor mode automatically.

