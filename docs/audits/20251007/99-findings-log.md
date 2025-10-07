### F-ECS-001 — Pre‑ECS ActorStore still present in server_core
**Severity:** P2  **Confidence:** High  **Area:** ECS/Data Model
**Context:** `ServerState` now uses `ecs::WorldEcs`, but `ActorStore` persists as an unused type.
**Evidence:** crates/server_core/src/actor.rs:58
**Why it matters (MMO best practice):**
- Duplicate authority surfaces increase accidental misuse risk.
- New contributors may target the wrong abstraction.
**Recommendation:** Remove `ActorStore` and related comments; keep only ECS types.
**Effort:** S  **Deps:** none  **Owner:** server_core

### F-ECS-002 — Wizard position mirroring bridge (sync_wizards)
**Severity:** P2  **Confidence:** High  **Area:** Server Systems
**Context:** Renderer wizard positions are mirrored into server ECS; PC is also respawned and casting resources reattached if missing.
**Evidence:** crates/server_core/src/lib.rs:200-270
**Why it matters (MMO best practice):**
- Violates pure server authority for player transforms.
- Respawn should be a server policy system, not tied to renderer positions.
**Recommendation:** Introduce movement intents and a `RespawnSystem`; deprecate then remove `sync_wizards`.
**Effort:** M  **Deps:** client movement plumbing  **Owner:** server_core/platform

### F-ECS-003 — Spatial grid rebuilt each tick
**Severity:** P2  **Confidence:** High  **Area:** Performance
**Context:** Grid rebuild O(N) per tick; projectile segment broad‑phase still scans actors; homing uses grid.
**Evidence:** crates/server_core/src/ecs/schedule.rs:70-83, 1040-1180
**Why it matters (MMO best practice):**
- Wastes CPU at scale; grid should be incremental and used for broad‑phase.
**Recommendation:** Move grid into ECS world and update on movement; provide segment/circle queries.
**Effort:** M  **Deps:** ECS write points  **Owner:** server_core

### F-ECS-004 — Legacy client AI/combat features remain (off by default)
**Severity:** P1  **Confidence:** High  **Area:** Architecture
**Context:** Legacy paths exist in renderer guarded by `legacy_*` features.
**Evidence:** crates/render_wgpu/src/gfx/renderer/update.rs:2035-2040; crates/render_wgpu/Cargo.toml (features)
**Why it matters (MMO best practice):**
- Extra code paths increase maintenance and risk of drift.
- Encourages accidental local testing divergence.
**Recommendation:** Delete features and code now that server authority is stable.
**Effort:** S-M  **Deps:** confirm all visuals/HUD from replication  **Owner:** graphics/client

### F-NET-007 — v3 deltas behind env flag
**Severity:** P3  **Confidence:** High  **Area:** Network/Replication
**Context:** Platform chooses v2 vs v3 via `RA_SEND_V3`. v3 has tests and interest mgmt.
**Evidence:** crates/platform_winit/src/lib.rs:330-420
**Why it matters (MMO best practice):**
- Two paths complicate testing and drift; prefer a single, tested default.
**Recommendation:** Default to v3 always; keep v2 encoder only for tooling until removed.
**Effort:** S  **Deps:** none  **Owner:** platform/net

### F-NET-005 — Client decodes legacy replication messages
**Severity:** P2  **Confidence:** High  **Area:** Network/Replication
**Context:** Client still decodes `NpcListMsg`/`BossStatusMsg` for compatibility.
**Evidence:** crates/client_core/src/replication.rs:162-180
**Why it matters (MMO best practice):**
- Dual formats complicate testing; actor snapshots should be the only source of truth.
**Recommendation:** Remove compatibility decoders; rely on v2/v3 actor snapshots.
**Effort:** S  **Deps:** ensure HUD paths read from actor views  **Owner:** client_core/ui

### F-OBS-006 — Ad-hoc env-based logging in server hot path
**Severity:** P3  **Confidence:** High  **Area:** Observability
**Context:** Conditional logging on RA_LOG_FIREBALL env in projectile spawn.
**Evidence:** crates/server_core/src/lib.rs:142-162
**Why it matters (MMO best practice):**
- Inconsistent observability; harder to aggregate and filter.
**Recommendation:** Migrate to `tracing` with per-system spans and metrics.
**Effort:** S  **Deps:** metrics/tracing crates  **Owner:** server_core
