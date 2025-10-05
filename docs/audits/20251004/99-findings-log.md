# Findings Log — 2025-10-04

### F-ARCH-001 — Renderer spawns server boss
**Severity:** P1  **Confidence:** High  **Area:** Architecture
**Context:** Renderer code triggers server `spawn_nivita_unique`, violating layering.
**Evidence:** crates/render_wgpu/src/gfx/npcs.rs:116; docs/audits/20251004/evidence/spawn-unique.txt
**Why it matters:**
- Renderer must remain presentation-only; creation belongs to server/app bootstrap.
- Tight coupling breaks web/native abstraction and complicates authority boundaries.
**Recommendation:** Move spawn calls to app/server init; renderer observes replicated state only.
**Effort:** S  **Deps:** server_core API  **Owner:** Graphics/Server

### F-ARCH-002 — Renderer hosts gameplay/input/AI and mutation
**Severity:** P1  **Confidence:** High  **Area:** Architecture
**Context:** `render_impl` performs input/controller updates, AI, and replication buffer apply.
**Evidence:** crates/render_wgpu/src/gfx/renderer/render.rs:12
**Why it matters:**
- Blends gameplay with draw; risks determinism and portability; hard to test.
**Recommendation:** Extract to `client_core` systems; keep renderer to upload/draw.
**Effort:** M  **Owner:** Graphics/Client

### F-NET-003 — Unbounded replication channel
**Severity:** P2  **Confidence:** High  **Area:** Networking
**Context:** `std::sync::mpsc` unbounded channel for replication.
**Evidence:** crates/net_core/src/channel.rs:13
**Why it matters:**
- Risk of unbounded memory growth; no backpressure under load.
**Recommendation:** Switch to bounded channel or ring buffer with drop/merge; add metrics.
**Effort:** S  **Owner:** Net/Core

### F-RENDER-004 — Hot-loop allocations/clones in renderer/server
**Severity:** P2  **Confidence:** High  **Area:** Renderer/GPU
**Context:** Frequent `.clone()` in hot paths across renderer and upload adapters.
**Evidence:** docs/audits/20251004/evidence/hotloop-allocs.txt
**Why it matters:**
- Per-frame heap churn and copies hurt perf; increase GC/allocator pressure.
**Recommendation:** Persist buffers, pass by reference, reuse bind groups/attachments; cache CPU meshes.
**Effort:** M  **Owner:** Graphics

### F-CI-005 — Test build failures (`cargo test --no-run`)
**Severity:** P1  **Confidence:** High  **Area:** Build/CI
**Context:** Unresolved imports in `collision_static`/`voxel_mesh` tests; missing dev-deps.
**Evidence:** docs/audits/20251004/evidence/warnings.txt
**Why it matters:**
- CI should compile tests to keep coverage green; surfaces integration regressions early.
**Recommendation:** Add dev-deps via `cargo add -p <crate> --dev core_units core_materials` etc.; fix fmt diffs.
**Effort:** S  **Owner:** Infra

### F-NET-014 — Snapshot versioning and caps missing
**Severity:** P2  **Confidence:** High  **Area:** Networking
**Context:** Messages lack a version/cap header; decode uses permissive defaults in one case.
**Evidence:** crates/net_core/src/snapshot.rs
**Why it matters:**
- Forward/backward compatibility and robustness require versioning and size caps.
**Recommendation:** Add version byte, size caps, strict decode errors; tests for malformed inputs.
**Effort:** S  **Owner:** Net/Core

### F-ASSET-007 — LFS patterns incomplete
**Severity:** P2  **Confidence:** High  **Area:** Assets/LFS
**Context:** Only models/anims tracked; textures/binaries not covered.
**Evidence:** docs/audits/20251004/evidence/gitattributes.txt
**Why it matters:**
- Large binaries bloat repo history; inconsistent LFS handling.
**Recommendation:** Track `*.bin, *.png, *.jpg, *.jpeg, *.ktx2` as needed.
**Effort:** S  **Owner:** Tools/Content

### F-DOCS-008 — Absolute paths in docs/scripts
**Severity:** P3  **Confidence:** High  **Area:** Docs
**Context:** Various docs reference `/Users/...` examples.
**Evidence:** docs/audits/20251004/evidence/absolute-paths.txt
**Why it matters:**
- Hurts portability; encourages copying absolute examples.
**Recommendation:** Replace with `$HOME/...` or env placeholders; keep script defaults via env.
**Effort:** S  **Owner:** Docs

### F-SIM-009 — `unwrap/expect` in server systems
**Severity:** P1  **Confidence:** High  **Area:** Server/Sim
**Context:** Multiple unwraps in destructible/projectile paths and telemetry config.
**Evidence:** docs/audits/20251004/evidence/panics-server.txt
**Why it matters:**
- Panics crash servers; malformed data should be handled gracefully.
**Recommendation:** Return `Result` or handle errors with metrics and defaults; avoid panics in prod.
**Effort:** M  **Owner:** Server

### F-DET-010 — Stable iteration guarantees
**Severity:** P3  **Confidence:** Med  **Area:** Determinism
**Context:** Hash maps used for meshes; ensure ordered iteration where it affects outcomes.
**Evidence:** crates/ecs_core/src/components.rs
**Why it matters:**
- Non-deterministic ordering across platforms/versions can diverge state.
**Recommendation:** Sort keys or use `BTreeMap`/stable index; keep queues deterministic.
**Effort:** S  **Owner:** Server

### F-OBS-011 — Metrics coverage gaps
**Severity:** P2  **Confidence:** Med  **Area:** Observability
**Context:** Limited metrics beyond a few counters.
**Evidence:** docs/audits/20251004/evidence/telemetry-uses.txt
**Why it matters:**
- Hard to enforce budgets and catch regressions without telemetry.
**Recommendation:** Add counters/histograms for tick time, budgets, net I/O, queue depth; dashboards.
**Effort:** S  **Owner:** Infra/Server/Client

### F-RENDER-012 — Unsafe surface lifetime transmute
**Severity:** P3  **Confidence:** Med  **Area:** Renderer/GPU
**Context:** Transmute for `wgpu::Surface` lifetime hacks.
**Evidence:** crates/render_wgpu/src/gfx/renderer/init.rs:84,91
**Why it matters:**
- UB risk if misused; future WGPU changes may invalidate assumptions.
**Recommendation:** Keep minimal, document rationale, prefer official patterns for surface recreation where possible.
**Effort:** S  **Owner:** Graphics

