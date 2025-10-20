# DragonProto Workflow — Programmatic Dragon Builds (Blender → GLB → Bevy)

This document describes the end‑to‑end, reproducible workflow to programmatically build, rig, and animate simple dragons in Blender (headless), export them as glTF/GLB with clean clips, and drop them into the Bevy vertical slice so they animate in‑engine. The goal is to stamp out a series of lightweight “proto‑dragons” for iteration and testing.

This doc synthesizes the authoring notes in the RedWyvern repo:
- BLENDER_AGENT_PLAYBOOK.md
- BLEND_ANALYSIS.md
- DRAGON_PROTO_EXPLAINED.md

…and adapts them to this repository’s conventions and engine constraints.

## TL;DR (golden path)

- Build + export in headless Blender:
  - `BL="/Applications/Blender.app/Contents/MacOS/Blender"`
  - `$BL --background --factory-startup -P scripts/make_dragon_proto_export.py`
  - Output: `exports/DragonProto.glb` (2 animations, 1 skin)
- Verify the GLB has 1 skin and ≥1 animations (optional snippet below).
- Add into this repo under `assets/models/DragonProto.glb` (copy or symlink).
- Run `cargo run` — the Bevy slice loads `DragonProto.glb`, plays the first two clips, and alternates on completion. No cameras/lights are imported; helper meshes are pruned by name.

## Authoring Requirements (engine‑side)

- Bevy 0.17 animation and glTF loader are used.
- Joint limit: rigs must have ≤ 256 joints for Bevy skinning.
- Clips: export each animation as a distinct clip (NLA tracks or per‑action), so the engine can pick/rotate clips easily.
- Keep cameras/lights out of the export; keep helper/control meshes out (or hidden).

## Build: Geometry → Rig → Skin → Clips (headless)

- Geometry: block out with `bpy.ops.mesh.primitive_*_add` (e.g., cylinder body + plane/cubes for wings). Optionally join into one mesh (`bpy.ops.object.join`) named `DragonProto_Mesh`.
- Rig: add an armature `DragonProto_Rig` with a minimal bone set (e.g., `spine`, `wing.R`, `wing.L`). Parent/child hierarchy is clean and simple.
- Skinning: select mesh, make armature active, parent with `ARMATURE_AUTO` for automatic weights. Ensure an Armature modifier is added and vertex groups exist per bone.
- Animation: create one Action per clip (e.g., `Flap`, `Bank`). Insert keyframes for pose bones; set bone `rotation_mode` (e.g., `XYZ`).
- Distinct clips: push each Action to NLA (headless‑safe via Data API). Each Action becomes its own NLA strip (`..._Track`).

Minimal NLA push (works in background):
```python
ad = arm_obj.animation_data or arm_obj.animation_data_create()
ad.use_nla = True
track = ad.nla_tracks.new()
track.name = f"{action.name}_Track"
fs, fe = [int(x) for x in action.frame_range]
strip = track.strips.new(action.name, fs, action)
strip.frame_end = fe
strip.extrapolation = 'HOLD_FORWARD'
arm_obj.animation_data.action = None
```

## Export: glTF/GLB settings (clean, headless)

Recommended baseline for simple rigs (Blender 4.x):
```python
bpy.ops.export_scene.gltf(
    filepath='exports/DragonProto.glb',
    export_format='GLB',
    use_selection=True,          # select only Mesh + Armature
    use_visible=True,            # hidden helpers not exported
    export_cameras=False,
    export_lights=False,
    export_extras=False,
    export_materials='NONE',     # switch to 'EXPORT' later as needed
    export_animations=True,
    export_frame_range=True,
    export_frame_step=1,
    export_force_sampling=True,
    export_bake_animation=True,
    export_animation_mode='NLA_TRACKS',  # distinct clips from strips
    export_nla_strips=True,
    export_merge_animation='NONE',
    export_anim_single_armature=True,
    export_def_bones=True,
    export_skins=True,
)
```

Notes
- Use selection + visibility to exclude cameras/lights and hidden helpers.
- If not using NLA, you can export the active Action by switching `export_animation_mode` to `ACTIVE_ACTIONS` and disabling NLA.
- For complex rigs/drivers, keep `export_force_sampling=True` and `export_bake_animation=True`.

## Verify the GLB (optional)

```bash
python3 - <<'PY'
import struct, json
p='exports/DragonProto.glb'
with open(p,'rb') as f:
  header=f.read(12)
  magic,version,length=struct.unpack('<III',header)
  cl, ct=struct.unpack('<II', f.read(8))
  assert ct==0x4E4F534A
  j=json.loads(f.read(cl).decode('utf-8'))
print('skins=', len(j.get('skins',[])))
print('animations=', len(j.get('animations',[])))
for i,a in enumerate(j.get('animations',[])):
  print(i, a.get('name'), 'channels=', len(a.get('channels',[])))
PY
```

Expect: `skins=1`, `animations>=1`.

## Integrate in this repo

- Put/ln the GLB under `assets/models/DragonProto.glb` (we currently symlink to the Desktop export for quick iteration).
- The Bevy slice loads GLTF with custom settings:
  - cameras/lights disabled (prevents stray light dots and camera entities)
  - animations enabled
- The runtime spawns `Scene0`, finds the existing `AnimationPlayer` in the spawned hierarchy, attaches an `AnimationGraph` with up to the first two clips, starts the first, and cycles to the second when finished.
- Helper meshes named like `cube`, `block`, `plane`, `grid`, `floor`, `helper`, `light`, `lamp`, `sphere`, `sun`, `emiss` are pruned.

## Naming and limits (engine compatibility)

- Joint count ≤ 256 for Bevy skinned meshes.
- Prefer lower‑case, `.` or `_` separated names for consistency.
- Clip names become GLTF animation names (we log and select by index today; can match by substring later).

## Making a series of simple dragons

- Parameterize geometry: body length, wing span/thickness, add optional `tail` or `head` bones (keep total joints under 256).
- Parameterize clips:
  - `Flap`: amplitude, frequency, phase
  - `Bank`: roll amount, hold time
  - Add more clips (`Glide`, `Hover`, `Dive`) as distinct Actions → NLA tracks.
- Export each variant to `exports/<proto-name>.glb` and link into this repo under `assets/models/<proto-name>.glb`.
- The app will pick the first two clips and cycle; we can extend to key‑triggered selection or a round‑robin of all clips.

## Troubleshooting

- “Extra boxes/dots” in engine: ensure export excludes cameras/lights and hidden helpers; we also prune by name at runtime.
- “No animations” in engine: ensure `export_animations=True`, clips are pushed to NLA or are active actions, and the GLB includes an `animations` array.
- Heavy rigs look wrong: for IK/driver‑heavy rigs, bake to a clean export rig first; for prototypes, keep rigs minimal.

## References

- RedWyvern authoring notes (external): BLENDER_AGENT_PLAYBOOK.md, BLEND_ANALYSIS.md, DRAGON_PROTO_EXPLAINED.md
- This repo’s Bevy slice: `apps/roa_slice_bevy/`
- Default loader settings applied in code: disable GLTF lights/cameras; enable animations; prune helpers; attach graph to GLTF’s `AnimationPlayer`.

