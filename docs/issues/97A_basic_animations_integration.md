# 97A — Integrate Basic/Universal Animation Library

Status: PROPOSED

Labels: assets, animations, retarget, renderer
Depends on: 97U (UBC characters), existing skinned animation pipeline (anim.rs)

Intent
- Import the “Animation Library[Standard]” pack and play its clips on the Universal Base Characters. Prefer the `.glb` (Godot) or `.gltf`-equivalent to avoid FBX parsing at runtime.

Source (local)
- Folder: `/Users/christopherdavid/Downloads/Animation Library[Standard]/`
  - Godot variant: `Godot/AnimationLibrary_Godot_Standard.glb` (preferred)
  - UE variant: `Unreal Engine/AL_Standard.fbx` (fallback; convert to glTF/GLB offline)
  - Unity variant: `Unity/AnimationLibrary_Unity_Standard.fbx` (same note as above)

Plan
- Use the Godot GLB as the source of animation clips. In this repo, a copy lives at `assets/anims/universal/AnimationLibrary.glb`.
- Extend or add a loader to parse glTF animation channels into `AnimClip` + tracks matching our `TrackQuat`/`TrackVec3` representation.
- Assume the animation pack skeleton matches UBC skeleton. If joint names diverge, add a small name-map retarget table to reorder tracks.
- Validate core clips (Idle, Walk, Run, Attack, Cast, Dodge) and wire to controller states for quick testing.

Files to touch
- `shared/assets` (crate `ra-assets`):
  - Add `load_gltf_animations(path) -> Vec<AnimClip>` that extracts node channels by joint name; group into named clips.
  - If clips reside in a single GLB timeline, segment by named animations (often stored as separate animations in glTF).
- `crates/render_wgpu/src/gfx/anim.rs`:
  - Allow switching active `AnimClip` per character; support different joint counts.
- `crates/client_core/src/systems/action_bindings.rs` and `controller.rs`:
  - Map movement speed to locomotion state (Idle ↔ Walk ↔ Run) and bind keys to “Attack/Cast/Dodge” clips for testing.
- `tools/` (optional):
  - A small offline script (Rust or Python) to convert FBX to glTF if we need specific UE/Unity variants; otherwise rely on provided GLB.

Tasks
- [ ] Copy `AnimationLibrary_Godot_Standard.glb` to `assets/models/anim/` (track via LFS if large).
- [ ] Implement or reuse glTF animation merge that produces `AnimClip` and per-joint track arrays aligned to the UBC skeleton.
- [ ] If necessary, define a `retarget.toml` mapping joint names from the animation GLB to UBC joint names; apply the mapping during load.
- [ ] Expose a simple clip registry: `Idle`, `Walk`, `Run`, `Attack`, `Cast`, `Dodge`.
- [ ] Drive `Idle/Walk/Run` from controller velocity; trigger `Attack/Cast/Dodge` from input bindings.
- [ ] Add CPU-only tests for animation sampling determinism on small rigs (e.g., 2–3 joints) modeled after the wizard tests.

Acceptance
- Animation clips load from GLB and play on UBC male/female with correct joint mapping.
- Locomotion transitions respond to movement speed; action clips trigger from input.
- Sampling produces stable palettes; tests pass under CI without GPU requirements.

Notes
- Viewer usage (for validation)
  - Load a UBC model first (male/female), then click `ANIMATIONLIBRARY` in the Library pane to MERGE its GLTF clips into the base model. If no base is loaded, the GLB loads as the base model.
  - Use `--ui-scale` to shrink UI text so long clip lists fit on screen (e.g., `--ui-scale 0.6`).
  - Use `--snapshot /tmp/out.png` for non‑interactive captures.
- Merge behavior
  - The viewer and loaders perform name‑based bone matching. If names differ, add `retarget.toml` and apply during merge.

Prep notes
- The `ra_assets::skinning::merge_gltf_animations` path already merges GLTF clips by node names and refreshes the viewer’s clip list. We will reuse this in engine code and add a retarget map when needed.
- If GLB clips include root motion, decide whether to consume or ignore it; for now, ignore (use controller for movement) and sample bone-local transforms only.
- License: include `License.txt` under `docs/third_party/` or append to `NOTICE` per policy; keep original filename.
