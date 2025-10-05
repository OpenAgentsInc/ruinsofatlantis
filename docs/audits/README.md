Below is a **turn‑key audit playbook** an agent can follow to assess the repo against large‑MMORPG best practices. It’s opinionated, step‑by‑step, and yields a consistent, reproducible report with hard evidence, scores, and prioritized fixes.

---

## 0) Goal & scope

**Goal.** Assess architecture, performance, determinism, network/replication, security/anti‑cheat, data & asset pipeline, observability, test/CI hygiene, and renderer/GPU practice—then produce a prioritized, actionable plan.

**Scope.** Entire repo: `crates/*`, `shared/*`, `tools/*`, `data/*`, `assets/*`, `docs/*`, build scripts, CI, and integration scripts (e.g., `xtask`).

**Non‑goals (this pass).** Feature design, art critique, balance tuning. (We only note risks/technical debt that block scale or stability.)

---

## 1) Outputs you must produce

Create a new folder and write *everything* there:

```
docs/audit/YYYY‑MM‑DD/
  00-exec-summary.md
  01-risk-matrix.md
  02-architecture.md
  03-network-replication.md
  04-determinism.md
  05-server-systems-and-scheduler.md
  06-ecs-and-data-model.md
  07-renderer-and-gpu.md
  08-asset-pipeline-and-lfs.md
  09-observability-and-ops.md
  10-security-and-anti-cheat.md
  11-build-ci-cd.md
  12-test-coverage-and-quality.md
  13-docs-and-governance.md
  99-findings-log.md
  evidence/
    ripgrep-*.txt
    cargo-metadata.json
    cargo-tree.txt
    clippy.txt
    fmt-diff.txt
    udeps.txt (optional)
    warnings.txt
    benches.md
    wgpu-validation.txt
    flamegraphs/ (optional)
```

**Finding template (use in `99-findings-log.md` repeatedly):**

```md
### F-<AREA>-<NNN> — <Short title>
**Severity:** P0 | P1 | P2 | P3  **Confidence:** High/Med/Low  **Area:** <Architecture/Net/Determinism/...>
**Context:** <1–3 sentences>
**Evidence:** <file:line refs, code snippets, command output excerpts>
**Why it matters (MMO best practice):** <1–4 bullets>
**Recommendation:** <specific change(s), who/where>
**Effort:** S/M/L  **Deps:** <if any>  **Owner:** <team/crate>
```

**Scoring rubric (0–5 per section):**

* **0** missing / broken; **1** ad‑hoc; **2** basic; **3** solid; **4** strong; **5** exemplary (at scale, with guardrails & automation).

---

## 2) Pre‑flight & ground rules

* **Do not run interactive windows.** Use the viewer’s `--snapshot` flag or offline tools only.
* **Time‑box any single command to ≤120s.** If it exceeds, log as an impediment and continue.
* Use **Rust stable** toolchain from `rustup`. Ensure `cargo`, `clippy`, `rustfmt`, `ripgrep (rg)`, `fd`, and (optional) `jq`, `hyperfine` are installed.
* Environment: run in repo root. Export:

  ```
  export RUSTFLAGS="-D warnings"
  export RUST_LOG=info
  ```

---

## 3) Inventory & architecture map

**Commands** (capture output into `evidence/`):

```bash
cargo metadata --format-version=1 > docs/audit/YYYY-MM-DD/evidence/cargo-metadata.json
cargo tree -e no-dev > docs/audit/YYYY-MM-DD/evidence/cargo-tree.txt
rg -n "workspace =|members =" -S Cargo.toml > docs/audit/YYYY-MM-DD/evidence/workspace-members.txt
```

**What to do**

* Build a crate DAG and annotate intended **layering**. For this repo the desired layering (adjust if docs say otherwise):

  * `shared/*` (utilities, asset loaders) **must not** depend on game ECS unless explicitly intended.
  * `data_runtime` **should be ECS‑agnostic** (note the recent coupling you saw).
  * `server_core` owns **authority**, simulation, budgets, scheduling, replication.
  * `client_core` owns prediction/UI; **no gameplay mutation**.
  * `render_wgpu` depends on client, never bootstraps server logic.
  * `net_core` is pure schema/channel code; **no gameplay**.
* **Flag violations** (e.g., `data_runtime -> ecs_core`), cyclic deps, or crates pulling in heavy deps they shouldn’t.

**Deliverable**: `02-architecture.md` (diagram + narrative + violations list).

---

## 4) Automated hygiene passes

**Format & lint**

```bash
cargo fmt --all --check 2>&1 | tee docs/audit/.../evidence/fmt-diff.txt
cargo clippy --all-targets --all-features -Z unstable-options --report-time \
  2>&1 | tee docs/audit/.../evidence/clippy.txt
```

* Summarize any lints that point to MMO‑scale risks: allocation in hot loops, needless clones, large enum size, panics on server paths, etc.

**Static greps** (store outputs under `evidence/`):

```bash
# panics and footguns (server_core especially)
rg -n "unwrap\(|expect\(|panic!\(" crates/server_core

# blocking and global mutability
rg -n "static mut|lazy_static!|OnceCell|thread::sleep|std::time::Instant::now\(\)" crates/server_core

# concurrency primitives (watch for Arc<Mutex> in hot paths)
rg -n "Arc<Mutex|Arc<RwLock|Mutex<|RwLock<" crates

# determinism traps on server
rg -n "HashMap<|HashSet<" crates/server_core
rg -n "rand::|thread_rng|random\(" crates/server_core

# allocation & copies in frame/tick loops
rg -n "collect::<Vec|to_vec\(\)|clone\(\)" crates/render_wgpu crates/server_core

# unsafe
rg -n "unsafe\s*\{" crates

# TODOs & FIXMEs
rg -n "TODO|FIXME|HACK|UNDONE" crates shared tools
```

Explain why each pattern matters at MMO scale (e.g., `HashMap` iteration nondeterminism, `unwrap!()` crash risk).

**Optional** (if available): `cargo udeps` for unused deps; record in `evidence/udeps.txt`.

---

## 5) Determinism & simulation tick

**What to examine**

* Fixed‑dt tick orchestration (budgeted carve→mesh→collider already present).
* Use of **floating point** and architecture differences (ARM vs x86).
* **Stable iteration** in authoritative systems (prefer `BTreeMap`/sorted keys or stable indices; avoid raw `HashMap` iteration in logic that must be deterministic).
* Randomness sources and seeding (no `thread_rng` on server simulation).

**Concrete checks**

* Grep for server systems that iterate `HashMap` and mutate simulation in that loop; log each site.
* Confirm a **single time source** for simulation (`dt` parameter), not `Instant::now()` calls inline.
* Confirm **collision & AI** resolution order is deterministic (pairwise loops use sorted indices).

**Deliverable**: `04-determinism.md` with a verdict and a list of exact sites to refactor for stable ordering.

---

## 6) Network, replication, interest & snapshots

**What to examine**

* `net_core::{snapshot,channel,interest}`.
* Encoding/decoding (bounds checks, versioning), drift resilience.
* **Interest management** complexity and correctness (SphereInterest & chunk_center helpers).
* Replication cadence & bandwidth budgeting (per‑tick size, per‑entity caps).

**Concrete checks**

```bash
rg -n "encode\(|decode\(" crates/net_core
rg -n "Snapshot|ChunkMeshDelta|BossStatus" crates/net_core crates/client_core crates/server_core
```

* Validate that decode paths are robust to malformed data (no panic).
* Verify **non‑blocking** drains and backpressure on channels (no unbounded growth in hot loops).
* Ensure **versioning** or feature bits exist (forward/backward compatibility plan).

**Deliverable**: `03-network-replication.md` with a table of message types, sizes (estimate), cadence, and interest policy. Add findings around versioning/backpressure.

---

## 7) Server systems, scheduler & budgets

**What to examine**

* `server_core::systems::{destructible,projectiles,npc}` and any job/scheduling code.
* Budget enforcement (mesh/collider budgets; tick ordering).
* Telemetry emit for budgets (already partially implemented).

**Concrete checks**

* Confirm all systems called from a single **scheduler** (or staged calls) with explicit budgets and that those metrics are recorded.
* Look for **O(N²)** or nested loops that will break at scale; note alternative data structures.

**Deliverable**: `05-server-systems-and-scheduler.md` with a table: system → complexity, budget knobs, metrics emitted, risks.

---

## 8) ECS & data model

**What to examine**

* `ecs_core::components` breadth & serialization strategy (`replication` feature).
* Component sizes & alignment (avoid large payloads in frequently updated components).
* Clean separation of **authoritative** vs **presentation** components.

**Concrete checks**

* Identify new components (Boss, LegendaryResist, Abilities…) and check they’re **not** used client‑side to mutate.
* Spot large vectors on hot components (e.g., `Vec<SpellId>` copied per tick).

**Deliverable**: `06-ecs-and-data-model.md` with a component inventory and hot‑path size analysis.

---

## 9) Renderer & GPU practice

**What to examine**

* `render_wgpu` pipelines, bind‑group layouts, submesh draws, buffer updates.
* Frame allocations vs persistent buffers (no per‑frame heap churn).
* Validation layers and error handling (surface lost, OOM).

**Concrete checks**

```bash
rg -n "create_buffer|write_buffer|create_texture|write_texture" crates/render_wgpu
rg -n "CommandEncoder|RenderPass" crates/render_wgpu
```

* Call out any **per‑submesh per‑frame texture reuploads**; ensure textures are cached and only updated when content changes.
* Confirm **bind group reuse** rather than re‑create each frame.
* Verify **surface error** handling paths (reconfigure on `Outdated/Lost`).

**Deliverable**: `07-renderer-and-gpu.md` with “hot loop allocations” list and proposed caching.

---

## 10) Asset pipeline & Git LFS

**What to examine**

* `.gitattributes` coverage and correctness.
* Asset directory layout; relative paths vs absolute.
* Loader robustness (UBC multi‑material fixes in `shared/assets`).
* License files (`docs/third_party` / `NOTICE`).

**Concrete checks**

```bash
cat .gitattributes
rg -n "/Users/|Downloads" -S
```

* Ensure no absolute paths remain in code/docs.
* Verify **LFS tracked types** include `.glb`, `.gltf`, `.bin`, `.png`, `.jpg`, `.ktx2` (if used), and animation assets.

**Deliverable**: `08-asset-pipeline-and-lfs.md` with a checklist and any missing patterns.

---

## 11) Observability (logs/metrics/traces)

**What to examine**

* `docs/telemetry.md`, server/client telemetry init.
* Metrics naming, label cardinality, and emit rate.
* Log levels and targets (`tracing` vs `log` mixing).

**Concrete checks**

```bash
rg -n "tracing::|metrics::|opentelemetry" crates
```

* Flag any **high‑rate logs** in hot paths; propose metrics instead.
* Ensure **counters/histograms** exist for: tick time, system budgets, net bytes sent, replication queue lengths.

**Deliverable**: `09-observability-and-ops.md` with dashboard suggestions (tables of metric names & intended panels).

---

## 12) Security & anti‑cheat posture

**What to examine**

* Trust boundaries (server authoritative by design).
* Any client‑side mutation hooks; RPC validation points.
* Deserialization hardening; channel misuse; potential DoS on unbounded queues.

**Concrete checks**

* Grep for TODOs referencing validation.
* Ensure decode paths bound allocations (length checks, caps).
* Verify **pointer‑lock/input** code does not leak PII; no logging of raw input deltas at high rate.

**Deliverable**: `10-security-and-anti-cheat.md` with a simple threat model and top 5 mitigations.

---

## 13) Build, CI/CD, and release

**What to examine**

* `xtask ci`, any GitHub Actions/workflows, release scripts.
* Feature flags and build matrices (native/WASM; macOS/Linux/Windows).

**Concrete checks**

```bash
rg -n "xtask|CI|workflow|pipeline" --hidden -S
```

* Confirm CI enforces `-D warnings`, runs tests, and (optionally) WGPU shader validation.
* Recommend adding **artifact builds** (headless server binary), and a **reproducible build** step.

**Deliverable**: `11-build-ci-cd.md` with pass/fail and gaps.

---

## 14) Tests & coverage

**What to examine**

* Unit tests by crate; integration tests; deterministic test strategy.
* Benchmarks (if any).

**Concrete checks**

```bash
cargo test --all --no-run 2>&1 | tee docs/audit/.../evidence/warnings.txt
rg -n "#\[test\]" crates shared tools
rg -n "#\[bench\]|criterion" crates
```

* Note flaky/long tests; suggest **determinism guards**.
* Call out **missing tests** for critical paths (decode, scheduler budgets, interest filter, renderer upload idempotency).

**Deliverable**: `12-test-coverage-and-quality.md` with coverage estimate (rough counts) and top 10 test gaps.

---

## 15) Docs & governance

**What to examine**

* `docs/issues/*` hygiene and status; runbooks.
* Architectural docs vs reality.

**Concrete checks**

* Ensure “do‑this‑next” reflects current reality.
* Verify third‑party license notes (SRD and custom content like “Soul Flay”).

**Deliverable**: `13-docs-and-governance.md` with freshness score and immediate cleanups.

---

## 16) Known hotspot prompts (paste these as you audit)

Use these **ripgrep prompts** to accelerate evidence collection (save outputs under `evidence/`):

* **Layering breaks:**

  ```
  rg -n "use ecs_core" crates/data_runtime shared
  rg -n "spawn_.*unique|spawn_nivita" crates/render_wgpu crates/server_core
  ```
* **Determinism suspects:**

  ```
  rg -n "HashMap<.*>\s*(for|iter|into_iter)\(" crates/server_core
  rg -n "thread_rng|random\(" crates/server_core
  ```
* **Hot loop allocations:**

  ```
  rg -n "(collect::<Vec|to_vec\(\)|clone\(\)|format!\()"
  ```
* **GPU churn:**

  ```
  rg -n "create_(buffer|texture|bind_group)" crates/render_wgpu | rg -n "frame|draw|update"
  ```
* **Logging flood:**

  ```
  rg -n "info!\(|debug!\(" crates | rg -n "for |loop|while"
  ```

---

## 17) Executive summary & risk matrix

In `00-exec-summary.md`:

* One paragraph per area with **score (0–5)** and **top 1–2 risks**.
* 90‑day roadmap: **Now (2 weeks)**, **Soon (30–45d)**, **Later (90d+)**.

In `01-risk-matrix.md`:

* Table of findings with **Severity × Likelihood**, link to each `F-…` entry.

---

## 18) Immediate high‑value checks for THIS repo (seeded by your recent work)

1. **Layering regression:** `data_runtime → ecs_core` introduced for string→enum parsing.
   *Expected*: move parsers next to `ecs_core` or `server_core::util`. Log as **F-ARCH-001**.

2. **Renderer spawns server boss:** move `spawn_nivita_unique` out of `render_wgpu` init to host bootstrap. **F-ARCH-002**.

3. **Immunity alias:** `"fear"` vs `Frightened` mapping—either change TOML or add alias. **F-DATA-003**.

4. **Saves derivation:** ensure CHA proficiency by default (INT/WIS/**CHA**). **F-SIM-004**.

5. **Hot loop allocations:** verify per‑frame bind group/texture creation in model‑viewer and renderer submesh draws; propose caching. **F-RENDER-005**.

Capture evidence and propose targeted patches for each.

---

## 19) Done criteria for the audit

* All section markdowns filled with a score (0–5), rationale, and **at least one recommendation**.
* `99-findings-log.md` has **numbered findings** with **evidence** and **actionable recommendations**.
* `00-exec-summary.md` contains a clear 30/60/90 day plan with owners per area.
* The audit runs **without interactive windows** and without any single command exceeding **120s**.

---

## 20) Optional: follow‑up automation

If you have time after the audit, propose an `xtask audit` subcommand that:

* Runs the commands above,
* Writes artifacts into `docs/audit/YYYY‑MM‑DD/`,
* Fails on P0 items (e.g., layering violations, panics on server paths).

---

### Quick start (copy/paste to begin)

```bash
AUD=docs/audit/$(date +%F); mkdir -p "$AUD/evidence"
cargo metadata --format-version=1 > "$AUD/evidence/cargo-metadata.json"
cargo tree -e no-dev > "$AUD/evidence/cargo-tree.txt"
cargo fmt --all --check 2>&1 | tee "$AUD/evidence/fmt-diff.txt"
cargo clippy --all-targets --all-features 2>&1 | tee "$AUD/evidence/clippy.txt"
rg -n "unwrap\(|expect\(|panic!\(" crates/server_core > "$AUD/evidence/panics-server.txt"
rg -n "HashMap<|HashSet<" crates/server_core > "$AUD/evidence/hash-collections.txt"
rg -n "create_(buffer|texture|bind_group)" crates/render_wgpu > "$AUD/evidence/gpu-creates.txt"
```

Then work through sections 3→15, pasting outputs into the `evidence/` folder and writing each section’s markdown with findings and scores.

---

If you want, I can also provide a **filled report skeleton** (pre‑written headings + checklists in each file) that the agent just populates with observations and evidence.
