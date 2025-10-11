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

Profession & Reputation (Design Summary)
- Worldsmithing is a player profession with level and reputation. Level unlocks creative power; reputation governs trust and distribution.
- How you level: publish spaces and earn XP from real engagement signals (unique plays, completion quality, return visits, lightweight reactions). Strong anti‑farm guardrails apply; fast‑fail sessions and bot loops don’t count.
- What you unlock: larger budgets (instance caps, map count), new verbs (from decor → triggers → NPCs/quests), and wider distribution (private → local → global). Titles/tier names are TBD; we’ll tune bands by testing.
- Reputation: slower, 90‑day rolling quality signal used for gates and visibility. Reports upheld reduce rep; rep decays slowly without fresh engagement.
- Publishing flow: auto validation (assets/links/perf smoke). First publish at a new band may require a light review; subsequent updates auto‑go live if they pass QA.
- Collaboration and social: co‑authors share credit; optional mentor co‑sign yields a small discoverability boost with shared accountability. Seasonal jams can grant temporary unlocks to everyone.
- Economy (non‑P2W): optional tips/patronage; blueprint vending for decor patterns earned by completing your space. Cosmetics only—no power creep.
- V1 fit: start with a “Garden Jam” (trees only) to exercise engagement, QA, and visibility without complex verbs. Use results to calibrate caps and unlock pacing.
