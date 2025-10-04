Status: Complete

Title: Replace PC wizard with UBC male and wire animations

Goal
- Replace the player character’s wizard model with the Universal Base Characters (UBC) male model.
- Keep existing wizard NPCs unchanged.
- Drive the PC with UBC animations (idle/walk/run/attack) similar to how zombies select clips.

Approach
- Load the UBC male GLTF as a separate skinned asset on the client (renderer), merge `assets/anims/universal/AnimationLibrary.glb` clips.
- Add dedicated GPU buffers for the PC (VB/IB/instance/palette) and a separate material bind group sourced from the UBC textures.
- Exclude the PC from wizard palette sampling and wizard instanced draw; draw the PC in its own pass using its own palette/material.
- Choose PC clips using fuzzy name matching similar to zombies and death knight:
  - Attack while casting; else Walk/Run when moving; else Idle.
  - Fallbacks: any idle-like clip → any other available clip → identity pose.

Acceptance
- PC renders as UBC male instead of wizard. Wizard NPC ring remains the same.
- PC animates (idle when stationary; walk/run when moving; attack during casts) using clips from the animation library.
- CI remains green (`cargo xtask ci`).

Notes
- Assets live under `assets/models/ubc/godot/Superhero_Male.gltf` and `assets/anims/universal/AnimationLibrary.glb` (tracked in LFS).
- Renderer retains wizard rig/instances for NPCs; only the PC path migrates to UBC.

Addendum (will fill after implementation)
Addendum (implemented on main)
- Renderer now loads the UBC male as a separate skinned asset for the player:
  - Fields added on Renderer for PC VB/IB, instance, palette, material, CPU rig, and prev-pos tracking.
  - File references: crates/render_wgpu/src/gfx/mod.rs, crates/render_wgpu/src/gfx/renderer/init.rs.
- PC animation palette update added with fuzzy clip selection (attack/walk/run/idle) akin to zombies/DK:
  - File: crates/render_wgpu/src/gfx/renderer/update.rs (`update_pc_palette`).
  - Called each frame from render loop right after wizard palette update.
- Wizard palette sampling now fills identity for the PC slot when a separate PC rig is active, avoiding stale data.
- Draw path updated to skip the PC instance in the wizard instanced draw, and to draw the PC with its own buffers/materials:
  - File: crates/render_wgpu/src/gfx/draw.rs (`draw_pc_only`, `draw_wizards`).
- Material creation reuses the existing wizard material helper per rig to bind baseColor textures from the respective GLTF.
- Animations merged from `assets/anims/universal/AnimationLibrary.glb`; logs indicate merged clip count.

How to verify (non-interactive)
- Set logs and run the app as usual; look for:
  - `PC: UBC male loaded: ...` and `merged GLTF animations from ...` lines.
  - Movement should switch PC clip to walk/run; casting should switch to an attack/cast clip when available.

Notes
- Wizard NPCs remain on the original wizard rig and draw path; only the PC uses UBC.
- If the UBC model fails to load, we fall back to the wizard rig for the PC automatically.

Follow‑ups
- Tune clip name preferences if you want specific locomotion variants.
- Optional: add UI/dev toggle to switch PC between male/female rigs at runtime.
