# Zone-Only Spawn Policy — Fully Remove Ad-Hoc Spawns

Context
- We still have demo-era spawn paths that inject NPC rings, bosses, and the ruins destructible outside of zone‑specific code. This bleeds into non‑demo zones (e.g., campaign_builder) and violates our ECS Architecture contract: server‑authoritative, data‑driven, and zone‑scoped.
- Goal: The only code that spawns gameplay actors or destructibles is zone‑specific (server_core::zones) driven by baked data (packs/zones/<slug>/snapshot.v1/*). Platform/renderer must not spawn gameplay, ever.

Problem Statement
- platform_winit historically spawned demo content on boot (NPC rings, boss, destructible). We partially gated it, but we need a principled, permanent cleanup.
- render_wgpu contains client‑side demo FX that can emit local projectiles (PortalOpen loop for NPC wizards). Even if presentation‑only, it must be disabled unless explicitly running the demo zone and driven by replicated state.
- Destructible instance + chunk deltas must be mounted by zone logic on the server, not piggy‑backed from demo paths.

Target End State
- Single source of truth for spawns: server_core::zones::{boot_with_zone, apply_zone_logic}.
- Zone snapshot + logic (trees.json, colliders.bin, logic.bin) inform initial actors and destructibles.
- platform_winit performs zero gameplay spawns; it only selects a zone and uploads zone batches to the renderer.
- render_wgpu remains presentation‑only; no projectiles/NPC spawns from client code.
- Demo content (wizard_woods only) implemented as zone logic data (or a minimal server_core::zones::wizard_woods module), not scattered ad‑hoc conditionals.

Work Items
1) Server zones module (authoritative)
   - Add `server_core::zones::{ZoneRegistry, boot_with_zone(slug), apply_zone_logic(World, Snapshot, Logic)}`.
   - Parse packs/zones/<slug>/snapshot.v1/logic.bin (JSON v1 OK) and spawn initial actors/destructibles server‑side only.
   - Provide helpers for common demo content (e.g., `spawn_ring_wizards(n, r)`), scoped to wizard_woods.

2) Remove platform spawns
   - Delete or fully gate all NPC ring/boss/destructible spawns in `crates/platform_winit/src/lib.rs`. Replace with a call to `server_core::zones::boot_with_zone(slug)` when in demo zone; no spawns otherwise.
   - WASM path: same behavior.

3) Remove renderer‑side gameplay emissions
   - In `crates/render_wgpu/src/gfx/renderer/update.rs`, delete or hard‑gate any code paths that emit projectiles or otherwise mimic gameplay (PortalOpen FX loop). Presentation may still sample bones to place VFX, but never create gameplay state.
   - Ensure renderer relies only on replicated state (ActorSnapshotDelta, Projectiles, HitFx).

4) Destructible ownership
   - Move `server_core::scene_build::add_demo_ruins_destructible` into zone logic for wizard_woods.
   - Client must only see `DestructibleInstance` + chunk deltas via replication from the server when that zone’s logic mounts them.

5) Zone data + bake
   - Extend `tools/zone-bake` to support minimal `logic.bin` schema for spawns (already emitting trees.json from scene spawns). Add support for initial actors and destructible proxies (IDs + AABBs) as data.
   - Document `scene.json` schema under `crates/data_runtime/schemas/zone_scene.schema.json`; wire into `cargo xtask schema-check`.

6) Picker + docs
   - Ensure Zone Picker displays only baked zones (no implicit demo). Wizard Woods remains a demo by virtue of its zone logic.
   - Update `docs/systems/zones.md` with the “zone‑only spawns” contract and the authoritative apply path.

7) Tests & Gates
   - Add unit tests under `server_core` for `boot_with_zone()` applying logic deterministically.
   - Add integration test that, when selecting `campaign_builder`, no server spawns are created unless present in its `logic.bin`.
   - Grep guards in CI:
     - Forbid `ring_spawn|spawn_wizard_npc|spawn_death_knight|spawn_nivita_unique` outside `crates/server_core/zones/**`.
     - Forbid `add_demo_ruins_destructible` outside `server_core/zones/**`.
     - Forbid any `spawn_` gameplay in `crates/platform_winit/**` and `crates/render_wgpu/**`.

8) Cleanup toggles
   - Remove ad‑hoc “demo” conditionals in platform/renderer. Replace with the single zone‑based policy.
   - If a local demo flag is desired, it must resolve to a demo zone slug, not flip hidden code paths.

Acceptance Criteria
- Running `campaign_builder`:
  - No NPC rings, no boss, no destructible instance unless defined in its zone logic.
  - No client‑side projectile emissions; renderer is presentation‑only.
- Running `wizard_woods`:
  - Demo content spawns exclusively from `server_core::zones::wizard_woods` logic applied on boot.
- CI enforces grep guards to prevent regressions.
- Docs updated (zones system + architecture guide mention spawn ownership rule).

References
- platform_winit demo spawns: crates/platform_winit/src/lib.rs
- renderer demo FX path: crates/render_wgpu/src/gfx/renderer/update.rs (PortalOpen loop)
- zones pipeline: docs/systems/zones.md, crates/data_runtime/src/zone_snapshot.rs, tools/zone-bake
- Architecture: docs/architecture/ECS_ARCHITECTURE_GUIDE.md (server‑auth, presentation‑only client)

