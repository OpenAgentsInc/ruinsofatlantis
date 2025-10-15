# Worldsmithing — In‑World Building Capabilities (Unified Spec)

Design overview (profession & reputation): see `docs/gdd/02-mechanics/worldsmithing.md`.

Purpose
- Worldsmithing is the in‑game capability to place, arrange, and persist world elements using the same runtime (camera, renderer, input) as regular play. It is not a separate editor; it’s a set of player/creator verbs gated by zone policy and permissions.

Crate & State
- Lives in `crates/worldsmithing/` with primary type `WorldsmithingState` and a fluent `Builder` for configuration.
- Replaced the older ad‑hoc `BuilderState` in `platform_winit`; the Campaign Builder integrates this crate directly.

Authoritative Model
- Server‑authoritative ECS orientation: the client issues worldsmithing commands; the server validates, applies, and replicates.
- Export/import paths are deterministic (fixed‑precision rounding, kind gating) to keep snapshots stable and shareable.

Renderer Tie‑ins
- “Ghost” preview for placement (semi‑transparent preview before commit).
- HUD/hotbar overlays and ghost preview draw via `render_wgpu` driven by platform input state.

Principles
- Single runtime: no separate “editor app.”
- Server‑authoritative persistence: authoring data becomes zone snapshots via the existing bake pipeline.
- Zone isolation: no global demo content; spawns belong to zones and their data only.
- Determinism and robustness: content flows through data → bake → snapshot; runtime draw degrades safely if assets are missing.
  - Content budgets: enforce per‑zone caps (e.g., trees ≤ 5,000). Warn at 80%; hard stop at 100% (toast + deny placement).
  - Asset catalog: Kind→Asset binding lives in data, not code (stable IDs, paths, optional scale).
  - Fallbacks are mandatory: always bind complete groups; use placeholders when assets missing.

Scope (V1)
- Capability: Place Tree only (instanced static foliage). No props/NPCs/triggers yet.
- Zone: `campaign_builder` (separate from `cc_demo` and `wizard_woods`).
- UI: Hotbar visible; slot 1 is “Place Tree.” Casting remains disabled by policy.
  - Authoring I/O: Export/import to `data/zones/<slug>/scene.json`; bake emits `packs/zones/<slug>/snapshot.v1/trees.json` (grouped by kind) used by the renderer.
  - Caps & QoL: `max_trees_per_zone = 5_000` (configurable), `max_place_per_second = 5`; one‑step Undo Last (`Z`).

Roles & Gating
- Player: normal play; no worldsmithing.
- Creator (zone author): can place trees in `campaign_builder`; toggles an overlay for guidance.
- Admin/GM (future): broader verbs and persistence hooks.
- Gating sources:
  - Zone policy (manifest): `show_player_hud` (hotbar visibility), `allow_casting` (kept false in `campaign_builder`), optional builder flags later.
  - Permissions (later phases): capability grants per session/player.

Player UX: Modes, Keys, HUD
- Modes: toggle Builder/Combat; Builder mode swaps the HUD to list worldsmithing kinds instead of spells and routes inputs to placement/rotate.
- Keys (baseline, IME‑safe):
  - Toggle: `B/C` (switch Builder/Combat)
  - Select kind: `1/2/3` (hotbar slots)
  - Place: `Enter` / Left Click
  - Rotate: `,` / `.` (wraps cleanly); `Q/E` or Wheel also supported
  - Overlay: `B` (help/caps)
  - Export/Import/Undo: `X` / `I` / `Z`

User Flow
- Entry: Select “Campaign Builder” from the Zone Picker.
- Ghost: Semi‑transparent tree follows ground under the crosshair; green = valid (normal.y ≥ 0.6), red = invalid. Fine rotate with Ctrl+Wheel (±1°). Optional jitter on confirm (±7.5°).
- Confirm: `Enter` / Left Click places an instance at the ghost pose; toast on success or validation warnings.

Public API (crate)
- Fluent builder: `Builder::new().caps(Caps).rules(Rules).build()`
- Rules (permissions/gating): `Rules { allowed_kinds: HashSet<String> }` — whitelist of placeable kinds (e.g., restrict to `tree.default` during bring‑up).
- `WorldsmithingState` capabilities:
  - `undo_last()` — single‑step undo of last placement
  - `cap_utilization()` — telemetry on remaining budget
  - `with_rules()` — swap/update ruleset at runtime; kind gating enforced at operation time

Authoring Data & Pipeline
- Catalog: `data/worldsmithing/catalog.json` (global) with optional per‑zone overrides mapping `kind` → `{ gltf, materials, scale }` and stable IDs.
- Authoring document (`data/zones/<slug>/scene.json`)
  - Minimal, human‑readable JSON with versioning and a `logic.spawns[]` list.
  - For V1, kinds are `tree.*` (e.g., `tree.default`). Fields include `id` (uuid), `kind`, `pos` `[x,y,z]`, `yaw_deg`. Serialize with fixed precision (3 decimals).
- Bake step (`tools/zone-bake`)
  - Transform `logic.spawns[]` (tree.*) into `snapshot.v1/trees.json` grouped by kind: `{ kind: "tree.default", instances: [ Mat4x4… ] }` (yaw + translation; scale baked into asset). Update meta counts and hashes.
- Runtime consumption (`data_runtime` → renderer)
  - `data_runtime` loads `trees.json` (optional) into the `ZoneSnapshot`.
  - `client_core`/renderer uploads per‑kind instance buffers and draws via the textured instanced pipeline.
  - Missing assets/textures fall back to safe placeholders. Log once per missing kind per attach.
- Schemas & CI: provide JSON schemas for authoring and snapshot; validate in CI; add a headless bake test.

Validation, Limits, Determinism
- Caps & rate limits prevent spam; surfaced via `cap_utilization()`.
- Deterministic export rounds to 3 decimal places to stabilize diffs and equality checks across runs.
- Import mismatch tolerance: map/kind mismatches log warnings but proceed when safe.

Renderer Expectations (V1)
- Instanced static draw path for trees; batch per kind/material.
- Textured instanced pipeline binds: globals, model, palettes, material (complete sets to avoid validation errors).
- Placeholders: DefaultMaterial/DefaultMesh for missing assets; never issue draws with incomplete bind groups.
- Performance target: foliage path handles hundreds of instances; prefer batched draws per kind/material.

Zone/Manifest Integration
- Manifest flags applied at startup:
  - `show_player_hud`: true in `campaign_builder` (exposes hotbar); may be false in `cc_demo`.
  - `allow_casting`: false in both `campaign_builder` and `cc_demo` (no spellcasting/projectiles).
  - `worldsmithing.kinds`: allow‑list of kinds available in the zone.
- Example block:
  ```json
  {
    "worldsmithing": {
      "enabled": true,
      "kinds": ["tree.default"],
      "caps": { "trees": 5000 },
      "hud": { "show_player_hud": true },
      "casting": { "allow_casting": false }
    }
  }
  ```
- Renderer/UI respect these flags consistently; no hardcoded scene checks.

Validation (Placement)
- Valid when the camera ray hits ground; slope threshold (normal.y ≥ 0.6); optional water/no‑place volumes in future.
- V1 ignores tree‑tree collision; future work may add separation/grid snap.

Robustness & Determinism
- No zone‑agnostic spawn logic; no ad‑hoc writes to `packs/` at runtime.
- All shaders validated in CI; bind groups match shader layouts.
- CPU‑only tests for data transforms; fixed precision on write for stable diffs/round‑trips.

Telemetry & Debug
- Log ability activation, placement confirms, and export/import events at info level (throttled).
- Counters: `placed_count`, `export_count`, `import_count`; measure export durations.
- Optional debug gizmo for the ghost hit point; disabled in non‑builder zones.
- Dev warning: if git‑lfs assets are missing for any used kind, emit guidance (e.g., “run git lfs pull”).

Proven Invariants (Tests)
- `rotation_wraps` — rotation keys wrap correctly (e.g., 359° → 0°).
- `caps_and_rate_limit_enforced` — quotas cannot be exceeded.
- `export_and_import_round_trip` — exported scenes re‑import without drift.
- `import_map_mismatch_warn_only` — tolerates nonfatal schema/kind diffs with warnings.
- `undo_last_removes_last` — undo is correct and idempotent.
- `rules_disallow_non_allowed_kinds` — placement blocked if not on allow‑list.
- `export_rounds_to_3dp` — numeric determinism guaranteed.

Current Integration Points
- `platform_winit`: drives window/input loop; switches Combat ↔ Builder UX; hosts IME‑safe input path; wires ghost preview tick.
- `render_wgpu`: renders hotbar/HUD and ghost previews; consumes state deltas.
- `net_core`: encodes/decodes snapshot/commands for server round‑trip.

Evolution (Post‑V1)
- Multiple tree kinds and by‑kind batching; simple palette to pick kinds via number keys.
- Props/NPCs/triggers authoring with server validation/replication.
- Builder permissions, rate limiting, and bounds checks.
- Terrain‑aware authoring; hot reload of snapshot assets; per‑kind asset caching.

Constraints & Non‑Goals (V1)
- No NPC/encounter spawns or combat verbs from authoring UI.
- No time‑of‑day/weather editing (manifest‑only).
- No networking replication of edits in V1; persistence flows via bake.
- No platform file pickers required; rely on conventional paths and CLI tools.

Known Issues / Recent Observations
- HUD regression: hotbar sometimes shows wizard spell slots in Builder mode instead of worldsmithing kinds (likely UI pipeline switch/policy propagation).
- Ghost preview visuals: rocks in ghost mode appear as flat splotches (material/normal/alpha path incomplete in preview pipeline).
- Initial white tree: one initial tree renders untextured at startup (stray placeholder instance or default material bound; remove or fix binding).
- IME‑safe keys (macOS): Builder keybinds not consistently triggering HUD updates/placements when IME is active; prefer physical scancodes in Builder context.

Terminology
- “Worldsmithing” is the feature/profession name in code/docs and UI.
- “Dungeon Master” is the role; “DMing”/“Dungeon Mastering” are verbs.

Acceptance Checklist (Creator POV)
- Select Campaign Builder → hotbar visible; slot 1 = Place Tree; casting disabled.
- Activate Place Tree → ghost appears on ground; rotate and confirm placement.
- Export → `data/zones/campaign_builder/scene.json` updated.
- Bake → `packs/zones/campaign_builder/snapshot.v1/trees.json` emitted and meta updated.
- Reload zone → trees render with textures; missing assets degrade safely; no HUD/casting regressions in other zones.
