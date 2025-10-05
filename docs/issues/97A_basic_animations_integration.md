# 97A — Integrate Basic/Universal Animation Library

Status: COMPLETE

Labels: assets, animations, retarget, renderer
Depends on: 97U (UBC characters), existing skinned animation pipeline (anim.rs)

Intent
- Import the “Animation Library[Standard]” pack and play its clips on the Universal Base Characters. Prefer the `.glb` (Godot) or `.gltf`-equivalent to avoid FBX parsing at runtime.

Source (local)
- Folder: `$HOME/Downloads/Animation Library[Standard]/`
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
- [x] Use `assets/anims/universal/AnimationLibrary.glb` as the source of clips (tracked via LFS).
- [x] Reuse glTF animation merge (`ra_assets::skinning::merge_gltf_animations`) to produce `AnimClip` and per‑joint tracks aligned to the UBC skeleton.
- [x] Viewer: add `--anim-lib <path>` to auto‑merge clips into the loaded model at startup; refresh animation list.
- [ ] Engine: expose a simple clip registry (`Idle`, `Walk`, `Run`, `Attack`, `Cast`, `Dodge`) and drive from controller inputs (follow‑up).
- [ ] Add CPU‑only sampling tests for a tiny rig (follow‑up).

Acceptance
- Animation clips load from GLB and play in the model viewer on UBC male/female with name‑based joint mapping.
- In the viewer, you can preview all clips via:
  - UI merge: click `ANIMATIONLIBRARY` to merge clips into the loaded UBC model.
  - CLI merge: pass `--anim-lib assets/anims/universal/AnimationLibrary.glb` alongside the model path.
- The animation list scales via `--ui-scale` so long clip names fit.

Notes
- Viewer usage (for validation)
  - Load a UBC model first (male/female), then click `ANIMATIONLIBRARY` in the Library pane to MERGE its GLTF clips into the base model. If no base is loaded, the GLB loads as the base model.
  - Use `--ui-scale` to shrink UI text so long clip lists fit on screen (e.g., `--ui-scale 0.6`).
  - Use `--snapshot /tmp/out.png` for non‑interactive captures.
- Merge behavior
  - The viewer and loaders perform name‑based bone matching. If names differ, add `retarget.toml` and apply during merge.

Prep notes
- The `ra_assets::skinning::merge_gltf_animations` path already merges GLTF clips by node names and refreshes the viewer’s clip list. We will reuse this in engine code and add a retarget map when needed.

Addendum — What shipped
- Model viewer CLI gained `--anim-lib <path>`; when combined with a UBC model path, it auto‑merges clips and refreshes the list.
- Library UI respects `--ui-scale` for both model entries and animation entries; long lists are easier to scan and click.
- Logs make merges explicit, e.g., `viewer: merged 142 GLTF animations from assets/anims/universal/AnimationLibrary.glb`.
- If GLB clips include root motion, decide whether to consume or ignore it; for now, ignore (use controller for movement) and sample bone-local transforms only.
- License: include `License.txt` under `docs/third_party/` or append to `NOTICE` per policy; keep original filename.
