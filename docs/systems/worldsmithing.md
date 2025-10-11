# Worldsmithing — In‑World Building Capabilities (Unified Spec)

Purpose
- Worldsmithing is the in‑game capability to place, arrange, and persist world elements using the same runtime (camera, renderer, input) as regular play. It is not a separate editor; it’s a set of player/creator verbs gated by zone policy and permissions.

Principles
- Single runtime: no separate “editor app.”
- Server‑authoritative persistence: authoring data becomes zone snapshots via the existing bake pipeline.
- Zone isolation: no global demo content; spawns belong to zones and their data only.
- Determinism and robustness: content flows through data → bake → snapshot; runtime draw degrades safely if assets are missing.

Scope (V1)
- Capability: Place Tree only (instanced static foliage). No props/NPCs/triggers yet.
- Zone: campaign_builder (separate from cc_demo and wizard_woods).
- UI: Hotbar visible; slot 1 is “Place Tree.” Casting remains disabled by policy.
- Authoring I/O: Export/import to data/zones/<slug>/scene.json; bake emits packs/zones/<slug>/snapshot.v1/trees.json used by the renderer.

Roles & Gating
- Player: normal play; no worldsmithing.
- Creator (zone author): can place trees in campaign_builder; toggles an overlay for guidance.
- Admin/GM (future): broader verbs and persistence hooks.
- Gating sources:
  - Zone policy (manifest): show_player_hud (hotbar visibility), allow_casting (kept false in campaign_builder), optional builder flags later.
  - Permissions (later phases): capability grants per session/player.

User Experience
- Entry: Select “Campaign Builder” from the Zone Picker.
- Hotbar: Slot 1 shows “Place Tree.” Activating it enters ghost placement.
- Ghost: Semi‑transparent tree follows ground under the crosshair; green = valid, red = invalid. Yaw rotates with Q/E (±15°) and Ctrl+Wheel (±1°).
- Confirm: Left Click or Enter places a tree instance at the ghost pose.
- Overlay (optional): B toggles a small helper overlay with controls, yaw, and placed count; keeps to policy (no cast bar/PC HUD in zones that disable it).
- Export/Import: X exports to data/zones/campaign_builder/scene.json; I imports from it. To see results in‑game, run the bake and reload the zone.

Input Policy
- No function keys. Suggested bindings:
  - 1: select “Place Tree.”
  - Q/E, Mouse Wheel: rotate ±15°; Ctrl+Wheel fine rotate ±1°.
  - Enter/Left Click: confirm placement.
  - B: toggle overlay.
  - X/I: export/import.

Authoring Data & Pipeline
- Authoring document (data/zones/<slug>/scene.json)
  - Minimal, human‑readable JSON with versioning and a logic.spawns[] list.
  - For V1, kinds are tree.* (e.g., "tree.default"). Fields include id (uuid), kind, pos [x,y,z], yaw_deg.
- Bake step (tools/zone-bake)
  - Transforms logic.spawns[] (tree.*) into snapshot.v1/trees.json with an array of column‑major 4×4 model matrices (yaw + translation; scale baked into the asset).
  - Updates meta.json counts (trees) and hashes; treat snapshot as the sole runtime source of truth.
- Runtime consumption (data_runtime → renderer)
  - data_runtime loads trees.json (optional) into the ZoneSnapshot.
  - client_core/renderer uploads per‑kind instance buffers and draws via the textured instanced pipeline.
  - Missing assets/textures must fall back to a safe placeholder (no invalid wgpu bindings).

Renderer Expectations (V1)
- Instanced static draw path used for trees (single mesh/material, many transforms).
- Textured instanced pipeline binds: globals, model, palettes, material (complete sets to avoid validation errors).
- Fallbacks:
  - If GLTF import fails or textures are missing, bind a default material/texture and/or a UV cube mesh; never issue draws with incomplete bind groups.
- Performance target: foliage path should comfortably handle hundreds of instances; prefer batched draws per kind/material.

Zone Policy Integration
- Manifest flags applied at startup:
  - show_player_hud: true in campaign_builder (to expose the hotbar); may be false in cc_demo.
  - allow_casting: false in both campaign_builder and cc_demo (no spellcasting/projectiles).
- Renderer/UI respect these flags consistently; no hardcoded scene checks.

Validation (Placement)
- Valid when the camera ray hits ground; optional slope threshold and water checks.
- V1 ignores tree‑tree collision (no min spacing). Future work can add separation or grid snap.

Robustness & Determinism
- No zone‑agnostic spawn logic. All spawns originate from zone data or server‑side rules for that zone.
- No ad‑hoc writes to packs/ at runtime; packs are build artifacts.
- All shaders validated in CI; textured instanced pipelines must bind declared groups in the same order as shaders.
- CPU‑only tests for data transforms (no GPU/window in CI); headless rendering is optional and out of V1.

Telemetry & Debug
- Log ability activation, placement confirms, and export/import events at info level (throttled).
- Optional debug gizmo for the ghost hit point; minimal and disabled in non‑builder zones.
- Add a one‑time warning for any missing asset kinds encountered during zone attach.

Testing (High‑Level)
- Data pipeline: Given a scene.json with N tree spawns, zone‑bake emits trees.json with N matrices and updates meta.json counts.
- Policy: Zones with show_player_hud=false hide HUD (including hotbar); allow_casting=false gates casting inputs; Campaign Builder shows hotbar but keeps casting off.
- Snapshot loader: ZoneSnapshot exposes trees as optional; unknown zones don’t crash or spawn content.
- Renderer (CPU‑side): Instance matrix upload and per‑kind grouping code paths do not panic and record intended flags/paths; asset missing → safe placeholder selected.

Evolution (Post‑V1)
- Multiple tree kinds and by‑kind batching; simple palette to pick kinds via number keys.
- Prop/NPC/triggers authoring with server‑authoritative validation and replication.
- Builder permissions (per‑player grants), rate limiting, and bounds checks.
- Terrain‑aware authoring: snap Y to terrain height at export time.
- Hot reload of snapshot assets for rapid iteration.
- Per‑kind asset caching to avoid re‑import hitches.

Constraints & Non‑Goals (V1)
- No NPC/encounter spawns or combat verbs from authoring UI.
- No time‑of‑day/weather editing (manifest‑only).
- No networking replication of builder edits; persistence flows exclusively via bake.
- No platform file pickers required; rely on conventional paths and CLI tools during V1.

Terminology
- “Worldsmithing” is the feature name used internally in code/docs and UI. There is no public‑facing brand separate from the game.

Acceptance Checklist (Creator POV)
- Select Campaign Builder → hotbar visible; slot 1 = Place Tree; casting disabled.
- Activate Place Tree → ghost appears on ground; rotate and confirm placement.
- Export → data/zones/campaign_builder/scene.json updated.
- Bake → packs/zones/campaign_builder/snapshot.v1/trees.json emitted and meta updated.
- Reload zone → trees render with textures; missing assets degrade safely; no HUD/casting regressions in other zones.

