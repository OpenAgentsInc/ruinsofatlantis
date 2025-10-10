# PC Animations

This document describes how the player character (PC) animations are selected and how to author new clips. The PC uses the UBC male rig merged with a shared animation library to keep behavior data‑driven and renderer‑only.

Overview
- Rig: UBC male (`assets/models/ubc/godot/Superhero_Male.gltf`) with clips merged from `assets/anims/universal/AnimationLibrary.glb`.
- Selection lives entirely on the client renderer. The server remains authoritative for gameplay. No gameplay logic is keyed off animation.
- Clip names are configurable in `data/config/pc_animations.toml` (and env overrides).

Phases & Mapping
- Idle/Walk/Sprint/Strafe (locomotion):
  - `idle = "Idle_Loop"`
  - `walk = "Walk_Loop"`
  - `sprint = "Sprint_Loop"`
  - `strafe = "Walk_Loop"` (strafe uses a walk cadence for readability)
  - Selection rules (priority): jump/cast → strafe‑only → sprint → walk → idle.
  - “Sprint” is forward‑only (W held, no strafing/backpedal).

- Jump (priority over locomotion):
  - `jump_start = "Jump_Start"` → `jump_loop = "Jump_Loop"` (airborne) → `jump_land = "Jump_Land"`.
  - Transitions are time‑based using clip durations.
  - Time‑scale: when sprinting, `jump_start` and `jump_loop` play ~30% faster.

- Cast (priority over locomotion; yields to jump):
  - `cast_enter = "Spell_Simple_Enter"`
  - `cast_loop  = "Spell_Simple_Idle_Loop"` (while channeling)
  - `cast_shoot = "Spell_Simple_Shoot"` (brief window at impact)
  - `cast_exit  = "Spell_Simple_Exit"`
  - Renderer starts a cast timeline on local cast begin, fires projectiles at `cast_time_s` from data/specs, then plays shoot/exit phase.

Configuration
- File: `data/config/pc_animations.toml`
- Env overrides (optional): `PC_ANIM_IDLE`, `PC_ANIM_WALK`, `PC_ANIM_SPRINT`, `PC_ANIM_STRAFE`, `PC_ANIM_CAST_ENTER`, `PC_ANIM_CAST_LOOP`, `PC_ANIM_CAST_SHOOT`, `PC_ANIM_CAST_EXIT`, `PC_ANIM_JUMP_START`, `PC_ANIM_JUMP_LOOP`, `PC_ANIM_JUMP_LAND`.

Implementation Notes
- Renderer samples palettes on CPU (`gfx/anim.rs`) and uploads to a dedicated PC palette buffer.
- Selection is state‑driven (`gfx/renderer/update.rs::update_pc_palette`) using small state flags:
  - Jump: `pc_jump_start_time`, `pc_land_start_time`.
  - Cast: `pc_anim_start` (enter/loop), `pc_cast_shoot_time` (shoot), `pc_cast_end_time` (exit).
  - Mouselook auto‑face: normal delay ≈ 0.25 s; while RMB held, delay ≈ 0.125 s.
- The legacy wizard rig (demo) still uses embedded clips (e.g., `PortalOpen`) for compatibility; the PC rig overrides via the config above.

Authoring Tips
- Add or retarget clips into `AnimationLibrary.glb` and update `pc_animations.toml` with the exact names.
- If a clip is missing, the renderer falls back using case‑insensitive substring matching (e.g., `walk`, `run`, `jog`).
- For explicit left/right strafes, provide `Strafe_Left_Loop` / `Strafe_Right_Loop` clips and we can split the `strafe` mapping.
