# Delegation: ensure SRD pdf is fully converted to markdown

_Generated 2025-10-06 12:46:30 for /home/christopherdavid/code/ruinsofatlantis_

## Objective
ensure SRD pdf is fully converted to markdown

## Grounding (Repo Evidence)
- Docs root: `docs`
- Status assessment: partially implemented (29 code matches)

### Docs snapshot
```
- docs/audits/README.md — ## 0) Goal & scope
- docs/graphics/bevy_gltf.md — # bevy_gltf: Notes and Takeaways
- docs/combat_sim_ecs.md — # Combat Simulation System — ECS Design and SRD Mapping
- docs/combat_sim.md — # Combat Simulation — Quickstart and Structure
- docs/delegations/20251006_115017_ensure-the-srd-docs-are-comprehensive-basedon-the-pdf.md — # Delegation: ensure the SRD docs are comprehensive basedon the PDF
- docs/delegations/20251006_115546_ensure-srd-pdf-is-fully-converted-to-markdown.md — # Delegation: ensure SRD pdf is fully converted to markdown
- docs/delegations/20251006_124143_ensure-srd-pdf-is-fully-converted-to-markdown-we-already-began-it.md — # Delegation: ensure SRD pdf is fully converted to markdown. we already began it
- docs/delegations/20251006_124444_ensure-srd-pdf-is-fully-converted-to-markdown.md — # Delegation: ensure SRD pdf is fully converted to markdown.
- docs/design/fevir-2025.md — # The core claim
- docs/design/spells/fire_bolt.md — # Fire Bolt (SRD 5.2.1) — Implementation Spec
- docs/graphics/gltf-animations.md — # glTF Skinning & Animations — What We Fixed
- docs/issues/0095_ecs_server_authority_plan.md — # Issue 95 — Server‑Authoritative ECS Refactor (Initial Execution Plan)
- docs/issues/0096_phase0_preflight.md — # Phase 0 — Preflight Hygiene and Feature Gates (Standalone Plan)
- docs/issues/100_nivita_of_the_undertide_unique_boss.md — # 100: Nivita of the Undertide — Unique Boss NPC
- docs/issues/101_zone_builder_editor.md — # Issue 101 — 3D Zone Builder (Editor + Bake Pipeline)
- docs/issues/95L_server_scene_build_destructibles.md — # 95L — Scene Build (Server): Data-Driven Destructible Registry
- docs/issues/95O_client_controller_camera.md — # 95O — Client Controller & Camera in client_core
- docs/issues/95P_tests_ci_expansion.md — # 95P — Tests & CI Expansion
- docs/issues/95Q_remove_legacy_client_carve.md — # 95Q — Remove Legacy Client Carve & Demo Paths
- docs/issues/95R_docs_adr.md — # 95R — ADR & Docs: ECS Server Authority & Jobs; Contributing
- docs/issues/95S_metrics_overlay_dev_toggles.md — # 95S — Metrics Overlay & Dev Toggles
- docs/issues/95T_scene_tagging_sample_content.md — # 95T — Data-Driven Scene Tagging & Sample Content
- docs/issues/96_telemetry_observability.md — # 96 — Production‑Grade Telemetry (Logging, Metrics, Tracing)
- docs/issues/97A_basic_animations_integration.md — # 97A — Integrate Basic/Universal Animation Library
- docs/issues/98_player_pc_ubc_male_integration.md
- docs/issues/99_clothing_system_wizard_robes.md
- docs/issues/ecs_refactor.md — ## 1) Phase 0 – Preflight hygiene & feature gates
- docs/issues/First_Playable_72h.md — # First Playable (72h) — Repo-Accurate Plan
- docs/issues/Lighting_M1.md
- docs/issues/Lighting_M2.md
- docs/issues/Lighting_M3.md
- docs/issues/Lighting_M4.md
- docs/graphics/lighting.md — # Ruins of Atlantis — Lighting Roadmap (engine‑agnostic)
- docs/design/spells/magic-missile.md — # Magic Missile (SRD 5.2.1) — Implementation Spec
- docs/old/wizard_viewer.md — # Wizard Viewer (Standalone)
- docs/research/hud.md — ## 1) Best practices in HUD design (engine‑agnostic)
- docs/research/hybrid-voxel-system.md — ## TL;DR (recommendation)
- docs/srd/README.md — # D&D 5E SRD 5.2.1 — Markdown Conversion
- docs/systems/controls.md — # Controls and Input Profiles
- docs/systems/frame_graph.md — # Frame Graph (Prototype)
- docs/systems/model_loading.md — # Model Loading — GLTF/GLB, Skinning, Submeshes
- docs/systems/sky_weather.md — # Sky & Weather
- docs/systems/spell_ability_system.md — # Spell & Ability System (MVP)
- docs/systems/terrain_biomes.md — # Terrain & Biomes (Phase 1)
- docs/systems/zones_system.md — # Zones System
- docs/observability/telemetry.md — # Telemetry — Logs, Metrics, Traces (dev usage)
- docs/ops/wasm-deployment.md — # WebAssembly (WASM) Deployment — Wizard Scene
- docs/ops/postmortems/web-wasm-blackout-postmortem.md
```

### Code/commit matches (excerpt)
```
crates/net_core/src/snapshot.rs:35:/// Keeping legacy per-message encodings intact, this leading tag ensures other
crates/ecs_core/src/components.rs:50:        anyhow::ensure!(
crates/ecs_core/src/components.rs:54:        anyhow::ensure!(
crates/render_wgpu/src/gfx/sky.rs:103:    /// - A tiny floor avoids fully‑black banding and keeps UI readable.
crates/net_core/src/lib.rs:23:        // Trivial smoke test to ensure the crate participates in CI.
crates/render_wgpu/src/gfx/mod.rs:1153:            // WGSL may round the struct size up; ensure our UBO is at least as large as the shader's view.
crates/render_wgpu/src/gfx/mod.rs:2027:        let near_lift = 0.5f32; // meters above anchor when fully zoomed-in
crates/render_wgpu/src/gfx/mod.rs:2028:        let near_look = 0.5f32; // aim point above anchor when fully zoomed-in
crates/render_wgpu/src/gfx/mod.rs:2654:        // Ensure SceneRead is available for bloom pass as well
crates/render_wgpu/src/gfx/mod.rs:2806:        // Pop error scope AFTER submitting to ensure validation covers command submission
crates/render_wgpu/src/gfx/mod.rs:3011:    fn ensure_proc_idle_clip(&mut self) -> String {
crates/render_wgpu/src/gfx/mod.rs:3228:                Some(self.ensure_proc_idle_clip())
crates/render_wgpu/src/gfx/ui.rs:1858:        // Purely CPU-side build check: ensure building adds some vertices
crates/render_wgpu/src/gfx/renderer/render.rs:388:    let near_lift = 0.5f32; // meters above anchor when fully zoomed-in
crates/render_wgpu/src/gfx/renderer/render.rs:389:    let near_look = 0.5f32; // aim point above anchor when fully zoomed-in
crates/render_wgpu/src/gfx/renderer/render.rs:1081:    // Damage numbers: update, queue, draw (independent of RA_OVERLAYS to ensure visibility)
crates/render_wgpu/src/gfx/renderer/render.rs:1211:    // Ensure SceneRead is available for bloom pass as well
crates/sim_core/src/sim/systems/ai.rs:1://! Simple AI: ensure bosses target an alive player; ensure players target boss.
crates/sim_core/src/sim/systems/ai.rs:34:    // Ensure players target the boss
crates/render_wgpu/src/gfx/renderer/update.rs:3:// use Debris via fully-qualified path
crates/render_wgpu/src/gfx/renderer/update.rs:15:// use destructible via fully-qualified path
crates/render_wgpu/src/gfx/renderer/update.rs:839:            // Burst a few batches to ensure visibility
crates/render_wgpu/src/gfx/renderer/update.rs:1124:        // Ensure per‑ruin proxy exists and is meshed (do not hold the &mut)
crates/render_wgpu/src/gfx/renderer/update.rs:2173:                        // Ensure NPC wizards resume casting loop even if all monsters are dead
crates/render_wgpu/src/gfx/renderer/update.rs:2582:        // Ensure initial spawn is terrain-aware.
crates/render_wgpu/src/gfx/renderer/update.rs:2925:            // Ensure NPC wizards resume casting loop even if all monsters are dead
crates/server_core/src/lib.rs:313:        // Ensure we mirror wizard positions
crates/server_core/src/lib.rs:929:        // Step forward a bit to ensure proximity explode triggers
crates/server_core/src/destructible.rs:338:            // Ensure effective values are sane regardless of source.

```

## Implementation Plan
- SRD status: 74 pages under `docs/srd/`, 0 TODO/Coverage markers.
- Coverage breakdown (matches; heuristic):
  - spells: 684
  - classes: 241
  - equipment: 367
  - rules_glossary: 119
  - combat: 81
  - adventuring: 649
  - conditions: 897
- Related delegations (recent):
``
docs/delegations/20251006_115546_ensure-srd-pdf-is-fully-converted-to-markdown.md
docs/delegations/20251006_124143_ensure-srd-pdf-is-fully-converted-to-markdown-we-already-began-it.md
docs/delegations/20251006_124444_ensure-srd-pdf-is-fully-converted-to-markdown.md
``
- Create `docs/srd/index.json` listing canonical SRD sections (slug, title).
- Add `tests/docs_srd_coverage.rs` to assert every index entry has a page and no TODO/WIP remain.
- Scaffold missing pages under `docs/srd/<slug>.md` with frontmatter + anchors; mark `Coverage: Partial` initially.
- Connect topical specs (e.g., `docs/design/spells/fire_bolt.md`, `docs/design/spells/magic-missile.md`) from the SRD ‘Spells’ pages.
- Add cross-links between ‘Rules Glossary’, ‘Combat’, ‘Conditions’, and related sections.
- Update Status lines in `docs/issues/` and record decisions.

## File-Level Tasks
- `docs/srd/index.json`: canonical SRD sections (slug, title).
- `docs/srd/README.md`: ensure index + coverage status; link to sections.
- `docs/srd/**`: scaffold missing pages; add anchors + cross-links.
- `tests/docs_srd_coverage.rs`: coverage + no-TODO/WIP checks.

## Test Plan
- `cargo test` runs coverage test; fails on missing `docs/srd/**` or TODO/WIP markers.
- Optional: add a simple script to diff `docs/srd/index.json` against `docs/srd/**`.
- Manual: navigate `docs/srd/README.md` links.

## Suggested index.json (minimal seed)
```json
{
  "sections": [
    { "slug": "rules-glossary", "title": "Rules Glossary" },
    { "slug": "combat", "title": "Combat" },
    { "slug": "adventuring", "title": "Adventuring" },
    { "slug": "conditions", "title": "Conditions" },
    { "slug": "equipment", "title": "Equipment" },
    { "slug": "spells", "title": "Spells" }
  ]
}
```

## Acceptance Criteria
- `docs/srd/index.json` exists and lists canonical sections.
- Every listed section has a corresponding `docs/srd/<slug>.md`.
- `cargo test` fails if TODO/WIP/Coverage markers exist under `docs/srd/**`.
- SRD pages link to existing topical specs where relevant.

## Immediate Next Steps
- Add `docs/srd/index.json` with the seed above.
- Create any missing `docs/srd/<slug>.md` from the list.
- Add `tests/docs_srd_coverage.rs` to enforce coverage + no TODO/WIP.

## Risks & Mitigations
- Documentation drift → enforce coverage in CI.
- Over-scoping → ship minimal pages first; iterate.
