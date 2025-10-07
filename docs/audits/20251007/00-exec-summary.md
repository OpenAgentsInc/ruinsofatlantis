# Executive Summary — 2025-10-07 (ECS Refactor Alignment)

Scope
- Full repo scan against the architecture in `docs/issues/ecs_refactor_part_2.md` with focus on: authoritative ECS world, ordered systems/schedule, event-based damage, spatial index, interest-managed replication, and removal of legacy client-side AI/combat.

Status (high level)
- Core server ECS is in place and largely matches the refactor doc: `server_core::ecs::WorldEcs` replaces the vec store, systems are ordered via a schedule, damage flows through events, a spatial grid exists, and actor-centric snapshots (v2) plus deltas (v3) with interest are wired in the platform loop.
- Primary deviations are legacy paths still present (feature-gated) and a few bridging APIs that should be retired once client input/prediction paths mature.

Top Deviations
- Legacy scaffolding remains in renderer and client: `legacy_client_ai`, `legacy_client_combat`, and compatibility decoders for `NpcListMsg`/`BossStatusMsg` are still in tree. See `crates/render_wgpu/src/gfx/renderer/update.rs:2035`, `crates/client_core/src/replication.rs:162`.
- `ActorStore` (pre‑ECS) still exists in code even though `ServerState` now uses `ecs::WorldEcs`. See `crates/server_core/src/actor.rs:58`.
- `ServerState::sync_wizards()` mirrors renderer wizard positions into the server ECS (bridge). Long‑term, the server should own wizard positions via input intents and prediction. See `crates/server_core/src/lib.rs:168`.
- Spatial grid rebuilds every tick rather than incremental updates on movement; acceptable now but not the end state for scale. See `crates/server_core/src/ecs/schedule.rs:323`.

Impact
- With defaults, the app honors server authority and replication; legacy code is off by default. However, nonessential legacy and bridge code creates maintenance overhead and risks accidental coupling.

Plan (condensed)
- Phase out legacy client AI/combat/features and delete unused pre‑ECS types.
- Replace `sync_wizards` position mirroring with intent-driven player movement in ECS schedule.
- Make spatial grid incremental and co-locate with ECS writes.
- Finalize replication by removing legacy list messages and keeping only v2/v3 actor snapshots.

