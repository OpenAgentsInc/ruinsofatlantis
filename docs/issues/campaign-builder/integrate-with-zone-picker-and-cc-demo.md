# Campaign Builder V1 — Integrate With Zone Picker + CC Demo

Context
- The earlier start.md assumed a generic engine and client-side gameplay. This issue anchors a concrete V1 on our codebase: server-authoritative ECS, Zone snapshots, and the existing Zone Picker.
- We introduce a new Zone: `campaign_builder` (separate from `cc_demo`). Use `cc_demo` as a UI/input baseline, but authoring/export lives under its own slug.
- Goals: a) ship a minimal in-app “Builder” overlay reachable when `campaign_builder` is selected from the Zone Picker; b) export authoring data to our Zone pipeline (`data/zones/<slug>/scene.json` → `tools/zone-bake` → `packs/zones/<slug>/snapshot.v1/logic.bin`); c) keep the client presentation-only (no gameplay mutation).

Out of Scope (V1)
- No live spawn of NPCs from the client; no network/replication changes.
- No 3D ghost mesh previews; V1 uses a simple screen-center placement with text overlay and optional flat gizmo ring later.
- No scene editing for time-of-day/weather (those live in `manifest.json`).

Integration Points (code today)
- Zone selection and boot: `crates/platform_winit/src/lib.rs` (Zone Picker model, boot modes, input routing).
- Zone client loader: `crates/client_core/src/zone_client.rs` and snapshot reader `crates/data_runtime/src/zone_snapshot.rs`.
- Renderer overlay hooks: `render_wgpu::gfx::Renderer::draw_picker_overlay(...)` and HUD overlay in `crates/render_wgpu/src/gfx/renderer/render.rs`.
- Zone bake tool: `tools/zone-bake` writes `packs/zones/<slug>/snapshot.v1/*` including `logic.bin`.

What We’re Building (V1)
- “Builder Mode” for `campaign_builder`:
  - Toggle with `B` after loading `campaign_builder` (also enable when `RA_BUILDER=1`).
  - Place simple “logic spawn markers” at the screen-center ground plane (Y=0) with yaw steps (`Q/E` ±15°, `Ctrl+Wheel` ±1°).
  - Export the session to `data/zones/campaign_builder/scene.json` under a `logic.spawns[]` array that zone-bake packs into `snapshot.v1/logic.bin`.
  - Import existing `scene.json` to continue editing.

Design Constraints (align to ECS guide)
- Server-authoritative: Builder never mutates gameplay entities. It writes authoring data only.
- Client presentation-only: Any in-app visuals are overlays/gizmos; no replicated state changes.
- Deterministic pipeline: zone-bake remains the single path to snapshots; no ad-hoc runtime saves in `packs/`.

User Flow
1) Launch app → Zone Picker. Select “Campaign Builder (campaign_builder)”.
2) In `cc_demo`, press `B` to enter Builder Mode. Overlay shows instructions and current yaw/placement count.
3) Aim with camera; press `Enter` to add a spawn marker at the screen-center ray intersecting plane Y=0 (V1). Rotate with `Q/E` or wheel.
4) Press `X` to Export → writes/updates `data/zones/cc_demo/scene.json` (creates dirs if missing).
5) Optionally press `I` to Import → loads `scene.json` and lists N existing markers; overlay shows them as text rows (V1).
6) Run `cargo run -p zone-bake -- cc_demo` to bake `packs/zones/cc_demo/snapshot.v1/logic.bin`.

Data Contract (authoring → snapshot)
- Authoring file: `data/zones/<slug>/scene.json`
  - Minimal shape (V1):
    ```json
    {
      "version": "1.0.0",
      "seed": 0,
      "layers": [],
      "instances": [],
      "logic": {
        "triggers": [],
        "spawns": [
          { "id": "uuid-v4", "kind": "npc.wizard", "pos": [x,y,z], "yaw_deg": 270.0, "tags": ["wave1"] }
        ],
        "waypoints": [],
        "links": []
      }
    }
    ```
- Bake output: `packs/zones/<slug>/snapshot.v1/logic.bin` contains a compacted form of `logic` (V1: JSON bytes passthrough is acceptable). `meta.json.counts.logic_spawns` reflects the count.

Work Items (specific and staged)

Stage 1 — Builder overlay plumbing
- platform_winit: Add Builder state and inputs
  - Add `RA_BUILDER=1` env toggle. When set and zone=`cc_demo`, auto-enable Builder Mode after load.
  - Add key handling when boot mode is Running:
    - `B` → toggle Builder Mode.
    - `Q/E` and `Wheel` (±15°) with `Ctrl+Wheel` (±1°) → adjust yaw while in Builder.
    - `Enter` → confirm a marker at screen-center plane Y=0 with current yaw.
    - `I` → import from `data/zones/<slug>/scene.json` (if exists).
    - `X` → export to `data/zones/<slug>/scene.json` (create parent dirs).
  - File path helper (native only): mirror `tools/zone-bake` logic for workspace roots using `CARGO_MANIFEST_DIR` to resolve `../../data/zones/<slug>/scene.json`.
  - Draw overlay using `Renderer::draw_picker_overlay(title, subtitle, &lines, selected_idx=0)` with:
    - Title: `Campaign Builder`
    - Subtitle: `B toggle   Enter place   Q/E rotate   I import   X export   Esc back`
    - Lines: last N markers like `#12  npc.wizard  [10.2,0.0,-3.7]  yaw=270°  wave1`.

- renderer: Expose camera forward for simple plane pick
  - Add a tiny helper to compute a world-space ray from camera center and intersect plane Y=0; expose via a method on `gfx::Renderer` that returns `(pos: glam::Vec3, yaw: f32)` suggestion for placement. Keep this CPU-only and free of gameplay coupling.
  - Alternatively (minimal): compute the hit in platform_winit with a copy of the current view-proj from `Globals` via an accessor, and perform ray-plane intersection there.

Stage 2 — Zone bake path for logic
- tools/zone-bake
  - Parse `scene_json` to count `logic.spawns.len()` and set `meta.counts.logic_spawns` accordingly (currently hardcoded to 0).
  - Write `logic.bin` as the compacted encoding of `logic` (V1: write JSON bytes; future: binary).
  - Update `hashes.logic` to reflect actual `logic.bin` bytes.

- data_runtime
  - Add `schemas/zone_scene.schema.json` describing the `scene.json` shape above. Wire into `cargo xtask schema-check`.
  - No loader changes needed for V1 (snapshot reader already surfaces `logic.bin`).

Stage 3 — Server-side placeholder (non-blocking for V1 export)
- server_core (follow-up PR)
  - Add a tiny `zones::logic` module with a deserializer for `logic.bin` (JSON V1) and a no-op apply that logs found spawns on boot for the selected zone. Do not spawn entities yet.

Keybinding Notes (repo policy)
- Avoid function keys; use letters/digits. Proposed: `B` (toggle Builder), `Enter` (place), `Q/E` and Wheel (rotate), `Ctrl+Wheel` (fine rotate), `I` (import), `X` (export).
- Document these briefly in `src/README.md` under “Controls” when the feature lands.

Testing & Validation
- platform_winit unit test: simulate a minimal packs dir and loading `cc_demo`, ensure Builder export writes `data/zones/cc_demo/scene.json` with at least one `logic.spawns[]` entry.
- zone-bake test: given a `scene.json` with N spawns, `meta.json.counts.logic_spawns == N` and `hashes.logic` matches `logic.bin`.
- client_core no tests needed in V1 (all builder state local to platform), but keep logic small and isolated.
- Manual run: select `cc_demo` → place 3 markers → export → run `cargo run -p zone-bake -- cc_demo` → verify `packs/zones/cc_demo/snapshot.v1/logic.bin` non-empty and `meta.json` counts updated.

Follow-ups (post-V1)
- Visual gizmo: draw a small flat ring or cross at the placement point. Candidate: add a tiny “debug ring” draw in `gfx::fx`.
- Tooling: `tools/campaign-builder` wrapper that boots `platform_winit` with `ROA_ZONE=cc_demo` and `RA_BUILDER=1` for content authors.
- Logic application: server reads `logic.bin` and spawns/links entities on boot in `server_core::zones`.
- Scene assets: allow choosing `kind` from a small catalog (`npc.wizard`, `npc.zombie`, etc.) via number keys; bake as data, never branched in systems.

- Zone Picker and `campaign_builder`: crates/platform_winit/src/lib.rs:72, crates/platform_winit/src/lib.rs:168, crates/platform_winit/src/lib.rs:320
- Zone client loader: crates/client_core/src/zone_client.rs:14
- Snapshot loader: crates/data_runtime/src/zone_snapshot.rs:28
- Renderer overlays: crates/render_wgpu/src/gfx/renderer/render.rs:1872
- Zone bake tool: tools/zone-bake/src/main.rs:1, tools/zone-bake/src/lib.rs:1
