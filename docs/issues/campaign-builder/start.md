Below is a practical **V1 product + technical spec** for adding “move + plant a tree + export/import placements” to the **Ruins of Atlantis** Rust codebase.

---

## 1) Problem & Goals

**Goal:** Let a player walk around the existing scene, enter a simple “Plant Mode,” place a *single* tree type onto valid ground, rotate it (yaw), and **export** those placements to a shareable file that another user can **import** and see the exact same trees in their scene.

**Success criteria (V1):**

* Player can toggle Plant Mode.
* A ghost/preview tree follows the ground under the crosshair/mouse.
* Player can rotate the preview around the vertical (yaw only).
* Player confirms placement; a solid tree spawns at that transform.
* Player can **Export** all placed trees to a single file.
* Player can **Import** a file to spawn/restore those trees.
* File format is human-readable, versioned, and stable.

**Non-goals (V1):**

* No multi-tree catalog/selector (just one tree prefab).
* No scaling, pitch/roll, or terrain deformation.
* No deletion/editing of placed trees after placement (optional “undo last” is listed as a stretch).
* No networking/replication or multi-user editing.
* No save of entire scene graph; only the placed-tree list.

---

## 2) Player Experience

**Default controls (configurable via input map):**

* **WASD / Left stick** – move character (already present).
* **T** – toggle Plant Mode on/off.
* **Mouse** (or right stick) – look.
* **Left Click / RT** – confirm placement when Plant Mode is ON.
* **Q/E or Mouse Wheel** – rotate preview tree yaw in ±15° steps.
* **CTRL+Mouse Wheel** – fine rotation (±1° steps). *(Optional but easy)*
* **F5** – Export placements to file.
* **F9** – Import placements from file (opens file picker or loads last used path).
* *(Stretch)* **Z** – Undo the last placement in the current session (not persisted).

**HUD feedback:**

* When Plant Mode is ON:

  * Small “Plant Mode” chip in a corner with: current yaw angle, “Valid/Invalid” location, control hints.
  * Preview tree: tinted **green** when valid, **red** when invalid.
* After Export/Import: transient toast (“Exported to …/plantings/rua-plantings-YYYYMMDD-HHMMSS.json”; “Imported N objects from file”).

---

## 3) Scope & Assumptions

* Game runs with an ECS (e.g., Bevy/Fyrox/custom). Spec is engine-agnostic and uses generic ECS terms; a Bevy-oriented mapping is included in §8.
* Ground is represented by one or more static meshes flagged as **Ground** for raycast targeting.
* Coordinate system is right-handed with **Y up** and world units in **meters**.
* Scene ID (string) is known at runtime (e.g., `"tutorial_beach_01"`). It’s embedded in export files to warn on mismatched imports.

---

## 4) Data Model

### 4.1 Components

* **Tree** — marker for placed trees.
  Fields:

  * `kind: TreeKind` *(V1 has one variant: `Default`)*

* **Transform** — your engine’s transform (at minimum translation vec3 + rotation quat/yaw).

* **PlacementPreview** — marker for the ghost tree entity.
  Fields:

  * `yaw_deg: f32`
  * `is_valid: bool`

### 4.2 Resources (singletons)

* **PlantModeState**

  * `active: bool`
  * `current_yaw_deg: f32` *(initialized to 0.0)*
  * `preview_entity: Option<EntityId>`
  * `last_export_path: Option<PathBuf>`
  * `map_id: String` *(current scene id)*

* **TreeAssets**

  * `default_tree_prefab: PrefabHandle` *(mesh/material or scene/prefab ref)*

### 4.3 File Format (JSON, versioned)

**Schema name:** `rua.plantings.v1` (future-proofing for v2+).

```json
{
  "schema": "rua.plantings.v1",
  "map_id": "tutorial_beach_01",
  "coordinate_space": "world",
  "unit": "meters",
  "objects": [
    {
      "id": "92c9c71c-3ab0-4d9f-a7ad-2a2af84a8db4",
      "kind": "tree.default",
      "pos": [10.25, 0.00, -3.75],
      "yaw_deg": 270.0
    }
  ],
  "created_at": "2025-10-10T12:34:56Z",
  "engine_version": "0.1.0"
}
```

**Rust types (serde):**

```rust
#[derive(Serialize, Deserialize)]
struct PlantingFileV1 {
    schema: String,                // "rua.plantings.v1"
    map_id: String,                // current scene id
    coordinate_space: String,      // "world"
    unit: String,                  // "meters"
    objects: Vec<PlacedTreeV1>,
    created_at: String,            // ISO-8601
    engine_version: String
}

#[derive(Serialize, Deserialize)]
struct PlacedTreeV1 {
    id: String,                    // uuid v4 as string
    kind: String,                  // "tree.default" (V1)
    pos: [f32; 3],                 // world position (x,y,z)
    yaw_deg: f32                   // yaw only
}
```

---

## 5) Systems & Logic

### 5.1 Enter/Exit Plant Mode

* **TogglePlantModeSystem**

  * On `T`:

    * If off → set `active = true`; spawn preview (ghost) entity:

      * Mesh/scene uses the default tree prefab with *preview material* (semi-transparent).
      * Add `PlacementPreview { yaw_deg: state.current_yaw_deg, is_valid:false }`.
      * Initialize at camera center raycast hit (if any).
    * If on → set `active = false`; despawn preview.
  * Ensure input focus doesn’t block character movement; both should work.

### 5.2 Preview Update

* **UpdatePreviewFromCursorSystem** (each frame when `active`)

  * Cast a ray from the camera center (or mouse) into the scene.
  * Filter hits to **Ground** layer.
  * If hit:

    * Snap preview `translation = hit_point`.
    * Compute `is_valid = true` (V1 simple validity: on ground; not below water; optional min slope).
  * Else:

    * Keep last valid point or mark `is_valid = false`.
  * Update preview rotation to `yaw_deg` about +Y.

* **RotatePreviewSystem**

  * On **Q/E**: adjust `current_yaw_deg` ±15° (wrap 0..360).
  * On **Mouse Wheel**: same as Q/E.
  * On **CTRL + Wheel**: ±1°.
  * Apply to preview rotation.

### 5.3 Place Tree

* **ConfirmPlacementSystem**

  * On **Left Click/RT** and `PlantModeState.active`:

    * If preview `is_valid`:

      * Spawn a **Tree** entity using **default tree prefab**.
      * Copy `Transform` from preview (pos + yaw; zero pitch/roll).
      * Assign a UUIDv4 and store it as an engine-side tag (optional component `EntityIdTag(uuid)`).
      * Append an in-memory record (for export) to a `PlacedTreesBuffer` resource:

        * `{ id, kind:"tree.default", pos, yaw_deg }`
      * (Optional) Play small SFX/VFX.
    * If not valid: do nothing (show brief “invalid location” toast).

*(Stretch) UndoLastPlacementSystem: remove last spawned tree entity & buffer entry.*

### 5.4 Export / Import

* **ExportPlacementsSystem** (F5)

  * Read `PlacedTreesBuffer` → build `PlantingFileV1`.
  * `map_id` = `PlantModeState.map_id`.
  * `created_at` = current UTC ISO-8601.
  * Write JSON to user data dir:
    `…/RuinsOfAtlantis/plantings/rua-plantings-YYYYMMDD-HHMMSS.json`
  * If dir missing, create it; on success, update `last_export_path`.
  * Emit toast with full path.
  * **Note:** For determinism, values are serialized with fixed precision (e.g., 3 decimals) to avoid floating noise.

* **ImportPlacementsSystem** (F9)

  * Open file picker (or load `last_export_path` if present).
  * Parse JSON; verify `schema == "rua.plantings.v1"`.
  * If `map_id` mismatch with current scene:

    * Show warning toast: “File is for `<map_id>`; you are on `<current>`; import anyway?”
    * For V1 (no modals), simply proceed and place in world coordinates; the warning is informational.
  * For each object:

    * Validate `kind == "tree.default"`.
    * Spawn **Tree** entity with `Transform` from pos + yaw.
    * Add/replace entries in `PlacedTreesBuffer`:

      * If an `id` already exists in buffer, skip or duplicate? **V1 behavior:** always append (duplicates allowed).
  * Emit toast: “Imported N trees”.

**Error handling:** if file missing, malformed, wrong schema → toast error; no crash.

---

## 6) Validation Rules (V1)

* **Valid placement** if:

  * Ray hits a **Ground** collider/mesh.
  * (Optional) Ground normal within slope threshold (e.g., `normal.y >= 0.6` ⇒ ~53° max slope).
  * (Optional) Not submerged (if a water plane exists; otherwise ignore).
* **No collision check** between trees in V1 (keep it simple).
  *(Future: min separation radius & overlap test.)*

---

## 7) Persistence & Paths

* **Export location (cross‑platform):**
  Use `dirs` crate to choose a user data dir:

  * Windows: `%AppData%/RuinsOfAtlantis/plantings/`
  * macOS: `~/Library/Application Support/RuinsOfAtlantis/plantings/`
  * Linux: `~/.local/share/RuinsOfAtlantis/plantings/`
* Filenames are timestamped; UTF‑8 JSON; LF newlines.
* Use `serde` + `serde_json`; add small helper to pretty‑print (2 spaces) for readability.

---

## 8) Suggested Bevy Mapping (if you are on Bevy)

**Crates / plugins (new):**

* `rua_planting` — gameplay systems + preview + input.
* `rua_persistence` — serde types + export/import ops.

**Assets:**

* `assets/trees/default_tree.glb` (scene)
  Load once at startup, store in `TreeAssets`.

**Components (Bevy):**

```rust
#[derive(Component)] struct Tree { kind: TreeKind }
#[derive(Component)] struct PlacementPreview { yaw_deg: f32, is_valid: bool }

enum TreeKind { Default }
```

**Resources:**

```rust
struct PlantModeState {
    active: bool,
    current_yaw_deg: f32,
    preview_entity: Option<Entity>,
    last_export_path: Option<PathBuf>,
    map_id: String,
}

struct PlacedTreesBuffer(Vec<PlacedTreeV1>); // mirrors file entries
struct TreeAssets { default_tree: Handle<Scene> }
```

**Systems (ordering):**

* `toggle_plant_mode_system` → `update_preview_from_cursor_system` → `rotate_preview_system` → `confirm_placement_system` → `export_import_systems`.

**Raycast:**

* Use `bevy_mod_raycast` or `bevy_mod_picking` (3D) with a “Ground” collision layer.
* Ray origin = camera; dir = forward (center of screen) or mouse-to-world ray.

**Spawning:**

* Preview: spawn a `SceneBundle` with default tree scene; override material to a transparent green or use a shader param; disable shadow casting for the preview.
* Confirm: spawn another `SceneBundle` with the same scene; set transform; add `Tree { kind: Default }`.

---

## 9) Telemetry & Debugging

* Log toggles and export/import actions at `info` level.
* Optional in‑world debug gizmo: draw a small ring at ray hit point when in Plant Mode.
* Developer console commands:

  * `plant.export <path?>` — export to optional path.
  * `plant.import <path>` — import explicit file.
  * `plant.clear` — *(dev only)* despawn all trees and clear buffer.

---

## 10) Acceptance Tests

1. **Place on ground:** Enter Plant Mode, preview turns green on ground; left‑click spawns one tree; it persists visually.
2. **Rotation:** Use Q/E and wheel; tree yaw matches HUD; exported yaw equals on-screen yaw.
3. **Invalid area:** Aim at the sky or non‑ground; preview turns red; click does nothing; no new buffer entries.
4. **Export file:** After placing ≥1 tree, press F5; JSON file is created; contains all objects with correct positions/yaws.
5. **Import file:** Restart the game (fresh scene), press F9, select file; all trees spawn at exact same transforms; buffer size matches file count.
6. **Map mismatch warning:** Import a file from another map; warning toast shows; trees still import to world coords.
7. **Error handling:** Import a corrupted file; toast shows “Invalid file / schema”; no crash.

---

## 11) Risks & Mitigations

* **Coordinate drift / precision:** Serialize with fixed precision and avoid cumulative transform edits; measure after import to verify drift < 1 mm.
* **Asset changes breaking import:** V1 has a single kind `"tree.default"`; future versions should maintain backward compatibility or provide a mapping table.
* **Scene differences:** Trees may appear underground if terrain differs; provide optional “raise to surface” snap on import (future).
* **Raycast performance:** Single ray per frame is trivial; no risk.

---

## 12) Future Extensions (post‑V1)

* Tree catalog (multiple kinds) + UI selector.
* Delete/Move tools; multi‑select; grid snap; minimum spacing.
* Full transform (pitch/roll/scale) and terrain alignment (align to normal).
* Steam Workshop or in‑game browser for sharing.
* Versioned upgrade path: `rua.plantings.v2` with richer metadata.
* Multiplayer co‑op placement and authoritative persistence.

---

## 13) Work Items (Engineering Checklist)

* [ ] Add `rua_planting` and `rua_persistence` crates/modules.
* [ ] Load `default_tree` prefab/scene into `TreeAssets`.
* [ ] Implement `PlantModeState`, preview spawning/despawning.
* [ ] Add raycast to ground + preview transform update.
* [ ] Implement rotation inputs and HUD chip/toasts.
* [ ] Implement placement: spawn tree + append to `PlacedTreesBuffer`.
* [ ] Implement serde types and JSON read/write helpers.
* [ ] Implement F5 export, F9 import, and toasts.
* [ ] Add minimal tests: round‑trip serialization, import spawns N trees.
* [ ] Docs: controls help and file format note for players/modders.

---

### Appendix A — Minimal JSON Example

```json
{
  "schema": "rua.plantings.v1",
  "map_id": "tutorial_beach_01",
  "coordinate_space": "world",
  "unit": "meters",
  "objects": [
    { "id":"8b2b5b5d-0f29-4f88-bd3e-0d81f0f9366a", "kind":"tree.default", "pos":[6.0, 0.0, -12.5], "yaw_deg": 45.0 },
    { "id":"e0c1f4f1-7a7e-4c1b-9a4c-b2f6a0bf915a", "kind":"tree.default", "pos":[12.3, 0.0, 3.7], "yaw_deg": 300.0 }
  ],
  "created_at": "2025-10-10T18:22:03Z",
  "engine_version": "0.1.0"
}
```

---

If you want this tailored to your exact engine stack (e.g., Bevy vs. custom ECS), I can translate §8 into concrete code skeletons for your setup and plug in specific crates for raycasting, UI, and file dialogs.
