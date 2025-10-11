### Worldsmithing (In‑World Creation)

Intent
- Let creators place world elements without leaving the game. No separate editor; the same camera, input, and renderer are used. Changes persist via the existing zone data pipeline.

Scope (V1)
- Verb: Place Tree (instanced static foliage) in the `campaign_builder` zone.
- HUD: Hotbar visible; slot 1 selects Place Tree. Casting remains disabled.
- Ghost preview snaps to ground; rotate Q/E or wheel; confirm with Left Click/Enter. Optional overlay (B) shows controls and counts.
- Export/Import authoring document (`scene.json`); bake → `trees.json` snapshot; reload to see results.

Guardrails
- Content caps per zone (e.g., ≤ 5,000 trees). Warn at 80%; hard stop at 100%.
- Valid placement requires ground hit and reasonable slope (normal.y ≥ 0.6). No‑place volumes reserved for future.
- Per‑zone policy governs HUD/casting; builder features do not leak into other zones.

Player/Creator Roles
- Player: normal gameplay; no editing.
- Creator: can place approved kinds (V1: trees) where the zone enables worldsmithing.
- Admin (future): expanded verbs and persistence tools.

Data & Persistence
- Authoring lives under `data/zones/<slug>/scene.json` with `logic.spawns[]` entries (`kind: "tree.*"`, `pos`, `yaw_deg`).
- Bake converts spawns into grouped transforms per kind and writes `packs/zones/<slug>/snapshot.v1/trees.json`.

Roadmap
- Add delete/move; multiple tree kinds and a palette; props/NPCs/triggers with server‑authoritative validation; collaborative creation with permissions.

