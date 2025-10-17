---
title: Renderer: Virtual Camera & Cinematography System
labels: area:gfx, type:Feature, prio:P1, docs-needed
assignees: 
milestone: 
---

## Context
We need a programmable virtual camera system to author cinematic shots (e.g., dragon fly‑bys, reveals, tracking, orbit, crane/dolly moves) and to sequence these into cutscenes. This enables in‑engine trailers, zone intros, and scripted storytelling while sharing core camera math with gameplay.

## Goals
- Author and play back camera shots with deterministic sampling and time control.
- Support common cinematography moves: cut, dissolve/blend, orbit, dolly/crane/track, follow/lead, look‑at with offsets, focal length/FOV ramps, and handheld noise.
- Compose sequences: chain shots with transitions, duration, and blocking events.
- Provide CPU‑side sampling APIs for tests and headless verification.
- Expose basic runtime controls: start/stop, scrub, speed, and loop.

## Non‑Goals
- Full timeline editor UI in this issue (basic debug UI only).
- Camera post stack (DOF/bloom) tuning; handled separately.
- Network replication of cutscenes; follow‑up task.

## Acceptance Criteria
- Camera track data model defined (keyframes/curves + targets) and serialized (RON/JSON) under `crates/data_runtime` compatible schemas.
- CPU sampler produces stable poses for fixed time inputs; unit tests cover bezier/ease curves, look‑at stability, and blend correctness.
- Runtime integration with renderer camera in `crates/render_wgpu/src/gfx/**`: can bind a cinematic camera that overrides gameplay camera.
- Shot graph supports: cut, cross‑fade blend, ease‑in/out; min set of moves: orbit, dolly, follow, look‑at with subject offsets.
- Debug overlay shows current shot name, time, and transition; hotkeys to play/pause/scrub documented in `src/README.md` per keybinding policy.
- Documentation added: module docs (rustdoc) and an overview in `docs/gdd/11-technical/overview.md` linking to camera system.

## Tasks
- [ ] Define data models and schemas for tracks/curves (position, rotation, FOV, target).
- [ ] Implement CPU sampler with curve interpolation and look‑at stabilization.
- [ ] Implement shot struct and transition blending (cut/cross‑fade/ease), with tests.
- [ ] Renderer hook: integrate cinematic camera path into frame build; toggle via flag.
- [ ] Minimal debug UI overlay + input hooks (play/pause/scrub), keys per policy.
- [ ] Unit tests: deterministic sampling for canonical paths; hash goldens.
- [ ] Docs: rustdoc for public APIs; add system overview and usage notes.

## Dependencies / Risks
- Must preserve determinism; no wall‑clock reads in hot paths (inject time).
- Performance budget: ≤ 0.1 ms CPU per frame for sampling.
- Coordinate conventions must match existing camera math; document any flips.

## Notes / Design Sketch
- Place core CPU types under `crates/client_core` (control glue) or a focused sub‑module in renderer if usage stays renderer‑local.
- Serialization lives in `crates/data_runtime`; consider a `cinema` schema folder.
- Curves: piecewise cubic (Catmull‑Rom or Bezier) with TCB or ease presets; quaternion slerp for rotation.
- Blends: parameterized by duration; lerp/slerp with curve easing.
- Subject targeting: entity id → world transform via ECS view; fallback to fixed target.

## Links
- docs/kanban.md
- src/README.md

