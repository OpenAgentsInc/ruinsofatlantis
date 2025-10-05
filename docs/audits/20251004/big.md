Awesome—thanks for the thorough audit. Below is a **copy‑pasteable set of GitHub issues** and **concrete implementation instructions** for your coding agent, mapped to the audit’s findings, risk levels, and the 30/60/90 plan. I’ve grouped work into **Now (due 2025‑10‑19)**, **Soon (due 2025‑11‑19)**, and **Later (target 2026‑01‑05)** milestones, and included acceptance criteria, test plans, and code/CI snippets where helpful.

> **Label pack to create (once):**
> `area:arch` `area:net` `area:render` `area:server` `area:ecs` `area:assets` `area:ci` `area:docs` `area:observability` `area:security`
> `risk:P1` `risk:P2` `risk:P3`
> `effort:S` `effort:M` `effort:L`
> `milestone:Now` `milestone:Soon` `milestone:Later`

---

## Meta‑Tracker (create first)

**Title:** Audit 2025‑10‑04 — Implementation Tracker
**Labels:** `area:arch` `risk:P1`
**Description:**
Use this issue to track all work items from the 2025‑10‑04 audit. Check off issues as they’re closed.

* [ ] # (to be filled) Stop server spawns from renderer (F‑ARCH‑001)
* [ ] # … Unbounded replication channel → bounded (F‑NET‑003)
* [ ] # … CI hygiene: fmt, tests, cargo‑deny (F‑CI‑005)
* [ ] # … Server `unwrap/expect` removal (F‑SIM‑009)
* [ ] # … Extract gameplay/input/AI from renderer (F‑ARCH‑002)
* [ ] # … Snapshot version headers + size caps (F‑NET‑014)
* [ ] # … Metrics coverage (F‑OBS‑011)
* [ ] # … LFS patterns for textures/buffers (F‑ASSET‑007)
* [ ] # … Deterministic iteration on HashMaps (F‑DET‑010)
* [ ] # … Hot‑loop clone reductions (F‑RENDER‑004)
* [ ] # … Absolute paths in docs/scripts (F‑DOCS‑008)
* [ ] # … Document/guard unsafe surface lifetime (F‑RENDER‑012)
* [ ] # … Interest mgmt & delta cadence; prediction/reconciliation; frame‑graph (Later)

---

## NOW (Due **2025‑10‑19**)

### 1) [P1] Stop server spawns & state mutation from renderer (F‑ARCH‑001)

**Labels:** `area:arch` `area:render` `area:server` `risk:P1` `effort:S` `milestone:Now`
**Why:** Renderer must be presentation‑only. Audit cites spawn at `crates/render_wgpu/src/gfx/npcs.rs:116` and mutation in `render_wgpu/src/gfx/renderer/render.rs:12`.
**Plan:**

1. **Move boss spawn** from `render_wgpu` to a server/app bootstrap path.

   * Create `server_core::boss::ensure_nivita_unique(world: &mut World) -> Result<EntityId, Error>`.
   * Call this from app/platform bootstrap (e.g., `platform_winit` or a top‑level app init), not from render code.
2. **Remove any world mutation calls** from renderer modules (`render_impl`, AI hooks, controller updates). For “Now”, focus on spawn removal and any direct server mutations you see in render modules.
3. **Layering guard:** Ensure `render_wgpu/Cargo.toml` has **no dependency** on `server_core`. Add an xtask assertion (see CI issue) that fails CI if `cargo tree -p render_wgpu | grep server_core` returns any match.

**Acceptance criteria**

* No references to `server_core` from `render_wgpu` (checked by xtask/CI).
* No calls to server entity creation/spawn functions from renderer files (grep passes).
* Game still spawns boss via app/server init; renderer only observes replicated state.

**Test plan**

* Unit: add a small test in `server_core::boss` ensuring “unique” spawn idempotency (calling twice results in a single unique entity).
* Integration: run a headless server tick smoke test that spawns boss at startup and exposes state to client.

---

### 2) [P1] CI hygiene: format, test build, cargo‑deny (F‑CI‑005)

**Labels:** `area:ci` `risk:P1` `effort:S` `milestone:Now`
**Why:** `cargo fmt --check` differs; `cargo test --no-run` fails on unresolved dev‑deps; `cargo deny` missing.
**Plan:**

1. **Fix current diffs**: run `cargo fmt --all`; commit.
2. **Repair dev‑deps** in crates with failing test builds (e.g., `collision_static`, `voxel_mesh`). Add necessary dev‑deps to `Cargo.toml` test modules (e.g., `core_units`, `core_materials` as needed).
3. **Introduce CI workflow** (see “Shared artifacts” below).
4. **Add cargo‑deny:** include `deny.toml` (shared below) and a CI step.

**Acceptance criteria**

* `cargo fmt --all -- --check` passes.
* `cargo test --workspace --all-features --no-run` passes.
* `cargo deny check` passes (no bans/advisories violations).

**Test plan**

* CI green on PR with the new workflow.
* Local build: `cargo xtask ci` mirrors workflow and passes.

---

### 3) [P2] Unbounded replication channel → bounded with backpressure (F‑NET‑003)

**Labels:** `area:net` `risk:P2` `effort:S` `milestone:Now`
**Why:** Current unbounded `std::sync::mpsc` can OOM under load.
**Plan:**

1. Replace `std::sync::mpsc` with a bounded channel (suggest `crossbeam-channel`).
2. Add non‑blocking `try_send`; on full, drop strategy = **drop newest** (simplest) and increment a drop counter (metrics).
3. Expose capacity via `NetConfig::replication_capacity` (default: 4096).
4. Emit metrics: `replication.enqueued_total`, `replication.dropped_total{reason="full"}`, `replication.queue_depth`.

**Implementation sketch (minimal):**

```rust
// crates/net_core/src/channel.rs
use crossbeam_channel as xchan;
pub struct ReplicationChannel<T> {
    tx: xchan::Sender<T>,
    rx: xchan::Receiver<T>,
    cap: usize,
}
impl<T> ReplicationChannel<T> {
    pub fn bounded(cap: usize) -> Self {
        let (tx, rx) = xchan::bounded(cap);
        Self { tx, rx, cap }
    }
    pub fn try_send(&self, msg: T) -> Result<(), xchan::TrySendError<T>> {
        match self.tx.try_send(msg) {
            Ok(()) => { metrics::counter!("replication.enqueued_total", 1); Ok(()) }
            Err(xchan::TrySendError::Full(m)) => {
                metrics::counter!("replication.dropped_total", 1, "reason" => "full");
                Err(xchan::TrySendError::Full(m))
            }
            Err(e) => Err(e),
        }
    }
    pub fn try_recv(&self) -> Option<T> { self.rx.try_recv().ok() }
    pub fn drain(&self, max: usize) -> impl Iterator<Item = T> + '_ {
        (0..max).filter_map(move |_| self.try_recv())
    }
    pub fn depth(&self) -> usize { self.rx.len() }
    pub fn capacity(&self) -> usize { self.cap }
}
```

**Acceptance criteria**

* All replication channels instantiated via `ReplicationChannel::bounded(...)`.
* Queue never exceeds configured capacity; metrics expose drops under stress.
* No `std::sync::mpsc` usage remains in `net_core`.

**Test plan**

* Unit: fill channel to capacity; assert `try_send` returns `Full`.
* Unit: depth/gauge sanity check.
* Integration: stress test with elevated snapshot rate; confirm drop counters > 0 with constrained capacity.

---

### 4) [P1] Remove `unwrap/expect` from server hot paths (F‑SIM‑009)

**Labels:** `area:server` `risk:P1` `effort:M` `milestone:Now`
**Why:** Panics crash servers; attackers can exploit malformed inputs.
**Plan:**

1. In `server_core/**`, replace `unwrap/expect` in non‑test code with result propagation (`?`) or validated defaults, logging+metrics.
2. Add crate lints:

   ```rust
   #![deny(clippy::unwrap_used, clippy::expect_used)]
   ```
3. For true invariants, keep `expect` but give actionable messages and count with a metric before panicking.
4. Audit `telemetry` config for fallible init; return `Result` to caller and degrade gracefully if exporter fails.

**Acceptance criteria**

* No `unwrap()`/`expect()` in `server_core` non‑test code (CI grep).
* Server continues to run on malformed input; increments error counters; no panics in normal operation.

**Test plan**

* Add fuzz/malformed decode tests for destructibles/projectiles to assert non‑panic behavior.
* Run a soak test with random invalid carve requests; verify uptime and error counters.

---

## SOON (Due **2025‑11‑19**)

### 5) [P1] Extract gameplay/input/AI from renderer → `client_core` (F‑ARCH‑002)

**Labels:** `area:arch` `area:render` `area:ecs` `risk:P1` `effort:M` `milestone:Soon`
**Why:** Clear layering: renderer uploads/draws; `client_core` handles input/prediction/AI.
**Plan:**

1. Create systems in `client_core` for input (controller/camera), simple AI, and replication buffer application.
2. `render_wgpu` consumes ECS data only; remove any input/AI logic.
3. Ensure `net_core` channel wiring lives in `client_core`/`server_core`, not in renderer.

**Acceptance criteria**

* `render_wgpu` contains no input/AI/replication application code (grep checks).
* Game still behaves identically in single‑player loop.
* `render_impl` limits to upload/draw orchestration.

**Test plan**

* Golden screenshot/frame timing tests to ensure no visual regressions.
* Unit tests for input system edge cases (zero dt, capped dt).

---

### 6) [P2] Snapshot version headers + size caps (F‑NET‑014)

**Labels:** `area:net` `risk:P2` `effort:S` `milestone:Soon`
**Why:** Needed for forward/back compatibility and robustness.
**Plan:**

* Add `const SNAPSHOT_VERSION: u8 = 1;` and `const SNAPSHOT_MAX_SIZE: usize = 256 * 1024;`.
* Frame each message as: `[version: u8][len: u32 LE][payload: len bytes]`.
* Reject unknown versions; hard‑cap `len` at `SNAPSHOT_MAX_SIZE`.
* Replace permissive `from_utf8(...).unwrap_or_default()` with a strict decode error that’s surfaced to metrics.

**Acceptance criteria**

* All snapshot encode/decode paths use framed messages with version+cap.
* Tests cover: wrong version, oversized length, truncated payload → decode error (no panic).
* Docs: a short table describing the frame format in `crates/net_core/README.md`.

**Test plan**

* Unit/property tests for framing.
* Fuzz with random bytes to ensure graceful errors.

---

### 7) [P2] Metrics coverage: budgets, tick time, net I/O, queue depth (F‑OBS‑011)

**Labels:** `area:observability` `risk:P2` `effort:S` `milestone:Soon`
**Why:** Enforce budgets and catch regressions early.
**Plan:**

* Counters: `boss.spawns_total`, `net.bytes_sent_total{dir}`, `net.bytes_recv_total{dir}`, `replication.queue_dropped_total{reason}`.
* Gauges: `replication.queue_depth`.
* Histograms: `tick.ms`, `mesh.ms`, `collider.ms`, `snapshot.size.bytes`.
* Export via existing Prometheus bootstrap.

**Acceptance criteria**

* `/metrics` endpoint exposes all metrics above.
* Dashboard: add a minimal Grafana panel JSON to `ops/` (optional if you prefer to configure externally).

**Test plan**

* Unit: assert histogram buckets receive updates in integration tests.
* Manual: run a local server, verify metrics increment while moving/attacking.

---

### 8) [P2] Hot‑loop clone reductions in renderer/upload paths (F‑RENDER‑004)

**Labels:** `area:render` `risk:P2` `effort:M` `milestone:Soon`
**Why:** Allocations and `.clone()` in per‑frame code hurt perf.
**Plan:**

* Replace `.clone()` on large `MeshCpu`/buffers with borrows; hoist temporary allocations out of loops.
* Reuse `wgpu::Buffer`s and bind groups; update with `queue.write_buffer`/mapped ranges.
* Cache CPU meshes and avoid re‑uploading if unchanged.

**Acceptance criteria**

* No `.clone()` on large types inside `render_impl` or per‑frame loops (grep for `.clone()` in those scopes).
* Frame time variance reduced in local benchmark scene.

**Test plan**

* Add a micro‑benchmark (feature‑gated) measuring per‑frame allocations; assert reductions after changes.

---

### 9) [P2] Asset LFS patterns for textures & binary buffers (F‑ASSET‑007)

**Labels:** `area:assets` `risk:P2` `effort:S` `milestone:Soon`
**Why:** Prevents repo bloat and inconsistent handling.
**Plan:**

* Update `.gitattributes` (see snippet in “Shared artifacts”).
* **Option A (no history rewrite):** only new files will be tracked; move existing large files to LFS gradually.
* **Option B (history rewrite):** run `git lfs migrate import` for affected extensions; coordinate with team.

**Acceptance criteria**

* `git lfs track` shows patterns for `*.png, *.jpg, *.jpeg, *.ktx2, *.bin`.
* `git lfs ls-files` lists existing large assets (Option B) or new assets (Option A).

**Test plan**

* Commit a test texture; verify LFS pointers on push.

---

### 10) [P3] Deterministic iteration over `HashMap` in server loops (F‑DET‑010)

**Labels:** `area:ecs` `area:server` `risk:P3` `effort:S` `milestone:Soon`
**Why:** Avoid platform/version‑dependent ordering.
**Plan:**

* Where order matters, collect keys & `sort_unstable()` before iteration, or use `BTreeMap`.
* Add comments noting determinism requirements.

**Acceptance criteria**

* No direct `for (k, v) in hash_map` in authoritative loops; replaced with sorted iteration.
* Two runs with same seed produce identical results hashes (add a simple state hash at end of tick).

**Test plan**

* Determinism test: run N ticks twice with the same seed; compare final hash.

---

### 11) [P3] Absolute paths in docs/scripts → portable examples (F‑DOCS‑008)

**Labels:** `area:docs` `risk:P3` `effort:S` `milestone:Soon`
**Plan:**

* Replace `/Users/...` and `C:\Users\...` with `$HOME/...` or `${PROJECT_ROOT}` placeholders.
* In scripts, rely on env var defaults (e.g., `SITE_REPO`) rather than absolute paths.

**Acceptance criteria**

* CI grep finds no absolute user paths in `docs/**` or `scripts/**`.

**Test plan**

* Add a CI step: `rg -n "/Users/|C:\\\\Users\\\\" docs scripts` must return empty.

---

### 12) [P3] Document/guard unsafe surface lifetime transmute (F‑RENDER‑012)

**Labels:** `area:render` `risk:P3` `effort:S` `milestone:Soon`
**Plan:**

* In `render_wgpu/src/gfx/renderer/init.rs`, document safety invariants above the `transmute`.
* Add a clear fallback path on `SurfaceError::Lost/Outdated` that re‑creates dependent resources.
* Consider feature gate or version guard if WGPU changes API.

**Acceptance criteria**

* Safety comment explains why UB is avoided; lost/outdated paths verified manually.

**Test plan**

* Force surface loss (resize/minimize) on supported platform; verify recovery without crash.

---

## LATER (Target **2026‑01‑05**)

### 13) [P2] Interest management + delta cadence budgeting

**Labels:** `area:net` `risk:P2` `effort:L` `milestone:Later`
**Plan:**

* Introduce AOI/interest regions to reduce snapshot size.
* Budget deltas per tick; coalesce updates for distant entities.

**Acceptance criteria**

* Snapshot size histogram shifts left for high‑entity scenes; tail percentiles reduced.

---

### 14) [P2] Client‑side prediction & reconciliation hooks

**Labels:** `area:arch` `area:net` `risk:P2` `effort:L` `milestone:Later`
**Plan:**

* Add prediction buffers in `client_core`; reconcile on authoritative updates; expose metrics for correction magnitude.

**Acceptance criteria**

* Jitter/jank reduced under 100–200ms simulated latency without divergence.

---

### 15) [P2] Lightweight frame‑graph in renderer

**Labels:** `area:render` `risk:P2` `effort:L` `milestone:Later`
**Plan:**

* Encode passes with explicit read/write edges; enable resource aliasing and hazard avoidance; prepare for job offload.

**Acceptance criteria**

* Clean pass graph definition; fewer redundant transitions/allocations; easier multi‑thread encoding.

---

### 16) [P2] Multi‑threaded job offload for meshing/colliders

**Labels:** `area:server` `risk:P2` `effort:M` `milestone:Later`
**Plan:**

* Convert synchronous closures in `JobScheduler` to a pool while maintaining deterministic scheduling (stable queues & bounded work).

**Acceptance criteria**

* Measurable server tick time reduction in destructible‑heavy scenes at same determinism.

---

## Shared artifacts (drop‑in snippets)

### A) GitHub Actions CI (`.github/workflows/ci.yml`)

```yaml
name: CI
on:
  pull_request:
  push:
    branches: [ main ]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Rustfmt
        run: cargo fmt --all -- --check
      - name: Clippy
        run: cargo clippy --workspace --all-features -- -D warnings
      - name: Tests (build only)
        run: cargo test --workspace --all-features --no-run
      - name: Cargo deny
        uses: EmbarkStudios/cargo-deny-action@v1
```

### B) `deny.toml` (at repo root)

```toml
[advisories]
vulnerability = "deny"
unmaintained = "warn"
yanked = "warn"
notice = "warn"

[bans]
multiple-versions = "warn"

[licenses]
allow = ["MIT", "Apache-2.0", "BSD-3-Clause", "Zlib", "ISC"]
confidence-threshold = 0.8
```

### C) `.gitattributes` additions (LFS)

```gitattributes
# Existing: models/anims tracked
assets/textures/**/*.{png,jpg,jpeg,ktx2} filter=lfs diff=lfs merge=lfs -text
assets/**/*.{bin}                               filter=lfs diff=lfs merge=lfs -text
```

> **If migrating history:** coordinate and run
> `git lfs migrate import --include="assets/textures/**/*.png,assets/textures/**/*.jpg,assets/textures/**/*.jpeg,assets/textures/**/*.ktx2,assets/**/*.bin"`

### D) Deterministic iteration snippet

```rust
let mut keys: Vec<_> = map.keys().copied().collect();
keys.sort_unstable();
for k in keys {
    let v = &map[&k];
    // ...
}
```

### E) Metrics macros (Prometheus)

```rust
use metrics::{counter, gauge, histogram};

counter!("boss.spawns_total", 1);
counter!("net.bytes_sent_total", bytes as u64, "dir" => "tx");
counter!("net.bytes_recv_total", bytes as u64, "dir" => "rx");
counter!("replication.dropped_total", 1, "reason" => "full");
gauge!("replication.queue_depth", depth as f64);
histogram!("tick.ms", tick_ms as f64);
histogram!("mesh.ms", mesh_ms as f64);
histogram!("collider.ms", collider_ms as f64);
histogram!("snapshot.size.bytes", size as f64);
```

### F) Framed snapshot (header) sketch

```rust
pub const SNAPSHOT_VERSION: u8 = 1;
pub const SNAPSHOT_MAX_SIZE: usize = 256 * 1024;

pub fn write_msg(mut w: impl std::io::Write, payload: &[u8]) -> std::io::Result<()> {
    if payload.len() > SNAPSHOT_MAX_SIZE { return Err(io_err("oversize")); }
    w.write_all(&[SNAPSHOT_VERSION])?;
    w.write_all(&(payload.len() as u32).to_le_bytes())?;
    w.write_all(payload)?;
    Ok(())
}

pub fn read_msg(mut r: impl std::io::Read) -> std::io::Result<Vec<u8>> {
    let mut ver = [0u8; 1]; r.read_exact(&mut ver)?;
    if ver[0] != SNAPSHOT_VERSION { return Err(io_err("bad_version")); }
    let mut len_buf = [0u8; 4]; r.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > SNAPSHOT_MAX_SIZE { return Err(io_err("len_cap")); }
    let mut payload = vec![0u8; len];
    r.read_exact(&mut payload)?;
    Ok(payload)
}
```

---

## One‑time “xtask ci” enhancement (optional but nice)

* Add an `xtask ci` subcommand that runs: fmt check → clippy → test (no‑run) → cargo‑deny → renderer/server layering guard (`cargo tree -p render_wgpu | ! grep server_core`).

---

## Grep checklist for the agent

* Layering: `rg -n "server_core" crates/render_wgpu` → **must be empty** after Issue 1.
* Unwraps: `rg -n "unwrap\(|expect\(" crates/server_core` → **must be empty** (non‑test).
* Clone hot loops: `rg -n "render_impl" -n --glob "!**/*_test.rs" | xargs -I{} rg -n ".clone\(" {}` → audit & fix.
* Absolute paths: `rg -n "/Users/|C:\\\\Users\\\\" docs scripts` → **empty**.

---

### What the coding agent should do first (sequence)

1. Open PR for **Issue 2 (CI hygiene)** with fmt fixes and the CI workflow.
2. Open PR for **Issue 3 (bounded replication channel)**.
3. Open PR for **Issue 1 (renderer spawn & mutation removal)**.
4. Open PR for **Issue 4 (server unwrap removal)**.
5. Move to **Soon** issues in parallel once “Now” is green.

---

If you’d like, I can tailor these issues to your exact repo structure (assignees, CODEOWNERS touches, or merging them into a single “Now” PR).
