Status: Proposed

Title: Initial Clothing System — Wizard Robes for UBC Characters

Problem
- Our UBC-based player/NPC characters (male/female) currently render as a single skinned body without wearable garments. We want a lightweight, deterministic, and renderer‑friendly way to dress characters in wizard robes that work with our existing animation set and pipeline.

Goals (MVP)
- Dress all UBC characters (male/female) in a wizard robe garment that animates correctly with our current skeleton and clips.
- Keep the runtime simple: no cloth simulation and no dynamic tearing/pins; the robe is a skinned mesh bound to the same skeleton.
- Avoid visible body poke‑through with a predictable, deterministic approach (masking under‑garment regions or using prepared body LODs with hidden areas).
- Preserve performance budgets (add ≤1 skinned submesh per character; stay within current skinning buffers/pipelines).

Non‑Goals (Future)
- Full outfit layering, blendshape tailoring, or cloth physics.
- Runtime garment authoring/tools; we’ll integrate pre‑authored robe meshes prepared for UBC rigs.

Background & Assumptions
- The “Universal Base Characters” (UBC) packs provide rig‑compatible male/female meshes with multi‑material submeshes. Our viewer already aggregates all submeshes for the dominant skin and merges animations from AnimationLibrary.glb.
- Garments that are authored for the same skeleton can be loaded as separate GLTF/GLB assets and bound to the same palette/sampling path (skinned matrices).
- To prevent poke‑through, common industry patterns are: (1) author a robe slightly offset and thicker, (2) hide body polygons under the garment via vertex masks/material slots, or (3) export a “body-with-robe” variant with under‑areas removed.

Proposed Design (Phase 1)
1) Asset structure
   - Place robe meshes under `assets/models/clothing/robes/wizard/` with UBC‑compatible skin (same joint names, bind pose).
   - Include materials (baseColor, normal, ORM). Use alpha mask for trims if needed; two‑sided only for thin edges.
   - Provide two variants initially (male/female proportions) if required by the vendor export; otherwise a single unisex if it binds correctly.

2) Runtime composition
   - Load the base UBC body and the robe GLTF for each dressed character.
   - Bind both to the same per‑character palette (same joint matrices); robe has its own material bind group and is drawn as an additional skinned submesh.
   - Masking: prefer asset‑level “body-with-robe” exports that remove under‑polygons. If unavailable, support a simple “body segment hide list” (e.g., node/primitive names) to skip drawing those submeshes on dressed characters.

3) Animation compatibility
   - No retargeting changes: robe tracks the same skeleton and joints.
   - The existing sampler (CPU palette generation) remains unchanged; the robe reads the same instance palette base.

4) Materials & variants
   - Start with one wizard robe material (baseColor+normal+ORM) with 2 colorways (e.g., Blue/Crimson). Add a small tint uniform if the material pipeline already supports it; otherwise ship two textures.
   - Keep alpha usage to masked trims; avoid full alpha blending for the body of the robe.

5) Renderer integration (scoped)
   - Extend the character draw path to accept an extra skinned submesh per dressed character: draw order body → robe → accessories (future).
   - Reuse existing skinned pipeline; ensure material BG for the robe is distinct from body.
   - Update the instance struct usage: robe shares `palette_base` with the body.

6) Data & config
   - Add a tiny config describing whether a character is “dressed” and, optionally, which robe variant/color to use. (File: `data/config/clothing.toml`, e.g., default=wizard_robe_blue.)
   - For now, dress all UBC characters by default (PC + sorceress NPC); wizard NPCs (legacy rig) remain unchanged.

7) Tooling (viewer)
   - Add toggles in the model viewer to load/unload the robe on UBC characters and switch color variants. This aids quick QA of poke‑through across clips.

Acceptance Criteria (MVP)
- PC (UBC male) and Sorceress (UBC female) render with a wizard robe that animates correctly with idle/walk/sprint/casting.
- No obvious poke‑through on standard poses/locomotion under our current camera distances.
- Default build compiles and passes CI; performance budgets remain within current limits (≤1 added skinned draw per dressed character).
- Viewer can preview a UBC model dressed with the robe and switch one color variant.

Implementation Plan (Phased)
P1 — Assets & composition
- Import robe GLTF(s) under `assets/models/clothing/robes/wizard/` with LFS.
- Add a minimal clothing descriptor (per-character: enabled + variant).
- Load robe asset alongside UBC body; create robe VB/IB + material BG.
- Draw robe after body with same palette base; keep shaders unchanged.

P2 — Masking / poke‑through mitigation
- Prefer “body-with-robe” meshes from export; if not, add a hide list for known body submeshes (eyes/eyelashes remain visible as needed).
- Verify across Idle/Walk/Sprint/Cast.

P3 — Viewer support
- Buttons to toggle Robe On/Off and Variant A/B; log any missing joints or materials.

Testing
- CPU-only unit tests (skinning):
  - Given a synthetic skeleton and two skinned meshes (body + robe) sharing joints, sample a clip and assert robe vertices transform consistently with body (hash of transformed positions for a known clip/time).
  - Validate that the hide list omits expected body submeshes for dressed characters.
- Integration spot-checks:
  - Sanity render under Idle/Walk/Sprint/Cast; snapshot in viewer.

Performance & Budgets
- Each dressed character adds one skinned submesh draw. Target ≤0.2 ms aggregate at current scene scale on mid‑GPU.
- Materials: 1 additional bind group per dressed character; negligible memory overhead compared to base body.

Risks & Mitigations
- Poke‑through in extreme poses — keep MVP to standard locomotion + casting; document known edge cases.
- Skeleton mismatches — verify joint names between robe and body; log actionable errors in viewer and at load.
- Material sorting — ensure robe uses the same pipeline (no blended body); avoid overdraw spikes.

Docs & Ownership
- Update `src/README.md` to mention clothing composition under gfx.
- Add a short `docs/systems/clothing.md` for skeleton compatibility, masking policy, and performance guidance.
- Owners: Graphics (render_wgpu), Assets (LFS), Tools team (viewer toggles).

Out of Scope / Future
- Full outfit sets and mix‑and‑match layering.
- Cloth simulation and runtime tailoring.
- Dynamic swaps at runtime (for now load-on-start only).

