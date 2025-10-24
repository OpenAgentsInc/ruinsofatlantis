# Baby Dragon Pumpkin Feast — Prototype Brief

## Pitch
- Halloween spin on the classic agar.io loop.
- You pilot a hungry baby dragon gliding above a haunted pumpkin patch.
- Eat treat-sized pumpkins to grow; level-ups unlock the ability to chomp tougher foes (skeletons at level ≥5, ghouls at level ≥10).
- Skeletons mob and swipe if you’re under-levelled; ghouls phase through and drain health until you overtake them.
- Ambient undead dance in the background to sell the “tech demo” spectacle for marketing.

## Tonight’s MVP Target
- One playable scene built on `apps/roa_slice_bevy` that proves the agar-style loop with the baby dragon and pumpkins.
- Minimal art pass: reuse existing dragon GLB, simple orange pumpkin meshes, greyboxing everywhere else.
- Flat lighting / default renderer only—no post-processing, DOF, or fancy VFX.
- Skeleton hazard as the single enemy type with a level gate; omit ghouls and dancing set dressing until later.

## Core Loop & Progression
- **Collect**: glide over bite-sized pumpkins; collision grants XP and scales the dragon uniformly (cap size for stability).
- **Avoid**: roaming skeletons immediately pop the player if level <5; once you ding level 5 you can bump them to clear space.
- **Grow**: three level bands (1-3, 4-6, 7+) that scale movement speed and collider radius; no extra VFX tonight.
- **Win/Lose**: simple HUD text (“Level X”, “Skeleton got you!”) and auto-reset on death; no cinematics.

## Technical Approach
- **Engine**: extend `apps/roa_slice_bevy` with a dedicated state module (`agar_dragon.rs`) that runs a top-down camera and 2D-ish movement on XZ.
- **Player Dragon**: load `DragonProto_v2.glb`, keep idle loop only; scale via `Transform::scale` based on XP. No animation graph cycling tonight if it costs time—we can lock to idle.
- **Pumpkins**: instantiate from a single placeholder mesh (orange material). Drop N pumpkins at spawn; respawn a new one at a random ring whenever eaten.
- **Skeletons**: spawn 2–3 using existing undead rig (idle + walk). Simple steering toward player with radius check for “kill” vs “get eaten”.
- **Collision & XP**: leverage existing ECS hitbox utilities; attach an `XpState` component with hard-coded thresholds `[0, 10, 25, 50]`.
- **UI**: minimal overlay via Bevy UI (`TextBundle`) for level, score, death notice.
- **Input**: reuse mouse/keyboard glide control from earlier slices or fall back to WASD impulse; keep friction simple.

## Absolute Needs (Tonight)
- Wire up the new scene module and register it behind a feature toggle or dev menu entry.
- Import/place a pumpkin mesh (placeholder cube with orange material acceptable).
- Hook dragon movement, XP accumulation, level-based scaling.
- Implement skeleton hazard logic with the level gate.
- Add minimal HUD text and restart flow.

## Tonight Game Plan (Rough Hours)
- **Hour 0–1**: carve out `agar_dragon.rs`, set up scene registration, lock top-down camera, load dragon + idle anim.
- **Hour 1–2**: implement WASD/analog glide, drop in pumpkins with simple respawn logic, hook XP + level scaling.
- **Hour 2–3**: spawn skeleton hazards, add kill/eat threshold, basic death/reset loop.
- **Hour 3–4**: polish pass—HUD text, tweak movement feels, ensure no crashes, record a quick screen capture.

## Risks & Mitigations
- **Animation setup time**: if the animation graph slows us down, lock the dragon to a single idle pose and revisit later.
- **Collision weirdness**: start with exaggerated collider radii and tune later; rely on debug prints before adding UI polish.
- **Spawn balance**: cap skeletons at three and pumpkins at ten to avoid ECS churn tonight.
- **Camera feel**: if follow camera jitters, freeze height/tilt and postpone smoothing.

## Stretch Ideas (If Time Appears)
- Add a single ambient music loop and pumpkin pop SFX.  
- Swap placeholder pumpkins for the emissive mesh.  
- Prototype ghoul behaviour behind a debug toggle.

This trims the concept to a bare-bones vertical slice we can stand up in one late session; everything else rolls forward once the core loop feels right.
