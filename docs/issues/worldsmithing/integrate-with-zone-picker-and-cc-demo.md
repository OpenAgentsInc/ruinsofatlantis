# Campaign Builder V1 — Integrated Builder (Hotbar + Ghost Placement)

Context
- This doc updates the older world‑builder notes to match our current direction: the “builder” is an in‑world capability, not a separate app. It uses the same client/runtime with per‑zone policy gating.
- Primary Zone: `campaign_builder` (separate from `cc_demo`). Use `cc_demo` as a starting reference only; authoring/export lives under its own slug and policy.
- Goals: a) show the standard hotbar in `campaign_builder` with slot 1 = Place Tree; b) when active, the ability enters a ghost placement mode; c) export placements into the zone pipeline (`data/zones/<slug>/scene.json` → `tools/zone-bake` → `packs/zones/<slug>/snapshot.v1/trees.json`). Client remains presentation‑only.

Out of Scope (V1)
- No live spawn of NPCs from the client; no network/replication changes.
- No non‑tree authoring (props/NPCs/triggers come later). Focus is trees only.
- No terrain sculpt/time‑of‑day editing (those remain in `manifest.json`).

Integration Points (code today)
- Zone selection and boot: `crates/platform_winit/src/lib.rs` (Zone Picker model, boot modes, input routing).
- Zone client loader: `crates/client_core/src/zone_client.rs` and snapshot reader `crates/data_runtime/src/zone_snapshot.rs`.
- Renderer overlays + foliage: `crates/render_wgpu/src/gfx/renderer/render.rs` and `crates/render_wgpu/src/gfx/foliage.rs` (ghost/instancing/textures).
- Zone bake tool: `tools/zone-bake` writes `packs/zones/<slug>/snapshot.v1/trees.json` (plus meta).

What We’re Building (V1)
- Integrated “Place Tree” ability in `campaign_builder`:
  - Hotbar is visible in `campaign_builder`; slot 1 = Place Tree. Casting remains disabled by policy.
  - Activating Place Tree enters a ghost placement mode: a semi‑transparent tree follows the ground under the crosshair. Valid = green tint; invalid = red.
  - Rotate yaw with `Q/E` (±15°) and `Ctrl+Wheel` (±1°). Confirm placement with Left Click or `Enter`.
  - Export to `data/zones/campaign_builder/scene.json` under `logic.spawns[]` with `kind: "tree.*"`. The bake step emits `snapshot.v1/trees.json` used by the renderer’s instanced foliage path.
  - Import existing `scene.json` to resume editing and pre‑populate placements.

Design Constraints (align to ECS guide)
- Server‑authoritative: Builder does not mutate gameplay entities; it writes authoring data only.
- Client presentation‑only: Ghosts and overlays are local; runtime world edits are not replicated.
- Deterministic pipeline: zone‑bake remains the one source of truth for snapshots; no ad‑hoc writes to `packs/`.
- Per‑zone policy: `campaign_builder` shows the hotbar but keeps casting disabled. `cc_demo` may share the same policy (HUD visible, casting off).

User Flow
1) Launch app → Zone Picker. Select “Campaign Builder (campaign_builder)”.
2) Activate hotbar slot 1 “Place Tree” (or press `B` to toggle a helper overlay).
3) Aim with camera; a ghost tree snaps to ground. Rotate with `Q/E` or wheel. Confirm with Left Click or `Enter`.
4) Press `X` to Export → writes/updates `data/zones/campaign_builder/scene.json` (creates dirs if missing).
5) Press `I` to Import → loads `scene.json` and lists existing spawns; overlay shows count and last few entries.
6) Run `cargo run -p zone-bake -- campaign_builder` to bake `packs/zones/campaign_builder/snapshot.v1/trees.json` (and update `meta.json`).

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
          { "id": "uuid-v4", "kind": "tree.default", "pos": [x,y,z], "yaw_deg": 270.0, "tags": [] }
        ],
        "waypoints": [],
        "links": []
      }
    }
    ```
- Bake output: `packs/zones/<slug>/snapshot.v1/trees.json` encodes model transforms (column‑major 4×4) optionally grouped by kind. `meta.json.counts.trees` reflects instance count.

Work Items (specific and staged)

Stage 1 — Hotbar + ghost placement (docs-only planning)
- Hotbar: Show in `campaign_builder`; slot 1 label “Place Tree”. Casting remains disabled by zone policy.
- Ability flow: Activate → ghost on ground, rotate `Q/E` or wheel; confirm with Left Click/`Enter`.
- Overlay: Optional helper overlay (`B`) with controls legend and placement count.

Stage 2 — Authoring I/O and bake
- Export/import: Read/write `data/zones/<slug>/scene.json` (`logic.spawns[]` entries with `tree.*` kinds).
- Bake: Convert spawns to `snapshot.v1/trees.json` (and update `meta.json.counts.trees`).
- Schema: Ensure `schemas/zone_scene.schema.json` includes `logic.spawns[].kind` = `tree.*`.

Stage 3 — Runtime render (already wired, docs alignment)
- Renderer: Consume `trees.json` and draw via textured instanced pipeline. Missing textures/models degrade gracefully to a safe placeholder.

Keybinding Notes (repo policy)
- Avoid function keys; use letters/digits.
- Proposed: `1` = Place Tree (hotbar), `B` toggle overlay, `Enter`/Left Click place, `Q/E` and Wheel rotate, `Ctrl+Wheel` fine rotate, `I` import, `X` export.
- Document these briefly in `src/README.md` under “Controls” when the feature lands.

Testing & Validation
- platform_winit test: load `campaign_builder`, simulate ability activation + confirm → export produces `data/zones/campaign_builder/scene.json` with ≥1 `tree.*` spawn.
- zone-bake test: given a `scene.json` with N `tree.*` spawns, output `trees.json` has N matrices; `meta.json.counts.trees == N`.
- Manual run: select `campaign_builder` → place 3 trees → export → run `cargo run -p zone-bake -- campaign_builder` → verify `packs/zones/campaign_builder/snapshot.v1/trees.json` and updated meta.

Follow-ups (post-V1)
- Visual gizmo polish: debug ring/grid snap; align to terrain slope.
- Tooling: `tools/campaign-builder` wrapper that boots with `ROA_ZONE=campaign_builder` and builder overlay on.
- Additional kinds: allow choosing multiple tree kinds via number keys and bake by‑kind groups.
- Triggers/NPCs: extend authoring to non‑tree kinds; keep server‑authoritative.

- Zone Picker and `campaign_builder`: `crates/platform_winit/src/lib.rs:72`, `crates/platform_winit/src/lib.rs:168`, `crates/platform_winit/src/lib.rs:320`
- Zone client loader: `crates/client_core/src/zone_client.rs:14`
- Snapshot loader: `crates/data_runtime/src/zone_snapshot.rs:28`
- Foliage/textured instancing: `crates/render_wgpu/src/gfx/foliage.rs:1`
- Renderer overlays: `crates/render_wgpu/src/gfx/renderer/render.rs:1872`
- Zone bake tool: `tools/zone-bake/src/main.rs:1`, `tools/zone-bake/src/lib.rs:1`
