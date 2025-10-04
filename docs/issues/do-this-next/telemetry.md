Great instinct. A one‑off `destruct_log!` macro is a band‑aid; for a production MMO you want a **coherent observability stack**: structured **logs**, low‑cardinality **metrics**, and end‑to‑end **traces**—with sampling, retention, and privacy baked in.

Below are the key considerations, then a repo‑aware GitHub issue you can drop straight into your tracker.

---

## What “production‑grade telemetry” means for an MMO

### 1) Use **`tracing`** (not bare `log`) for code‑level telemetry

* `tracing` gives you **structured fields** and **spans** (causal context). It integrates cleanly with OpenTelemetry.
* Keep logs for **rare/significant events** (errors, state transitions). Use **metrics** for high‑rate signals. Use **traces** for end‑to‑end latency & hotspots.

### 2) Separate **logs vs metrics vs traces** (and keep cardinality under control)

* **Logs**: JSON, structured (e.g., `zone`, `player_id`, `entity_id`, `destructible_id`, `chunk=(x,y,z)`), but avoid high‑cardinality values in *metrics labels*.
* **Metrics**: Counters, gauges, histograms. Labels (aka tags) must be **bounded** (`zone_id`, `shard_id`, `material_id`, `result={"ok","err"}`).
* **Traces**: Attach spans to **player commands**, **server ticks**, **jobs** (voxelize/mesh/collider). Export via **OTLP** to a collector (Jaeger/Tempo/etc).

### 3) Sampling & budgets

* **Never** log per‑voxel or per‑triangle at `info`. For hot loops:

  * Metrics: `voxel.chunks_meshed += 1`, `voxel.mesh_ms.observe()`.
  * Traces: sample 1–5% of heavy spans (tail sampling in the collector is ideal).
* Budget: capture tick duration, queue depths, and job ms; alert if thresholds are breached.

### 4) Correlation IDs & context

* Generate a **session_id** per player, **command_id** per input, **impact_id** per carve. Put them in spans and logs so you can pivot from a log to the exact trace.
* Propagate across client→server→jobs.

### 5) Privacy & safety

* Treat **player names/emails/IPs** as PII; do **not** put them in logs/metrics labels. Use numeric IDs.
* Redact payloads. Provide opt‑out for client telemetry. Keep retention short for raw logs.

### 6) Where to emit

* **Server** is the source of truth: exports logs/metrics/traces centrally.
* **Client**: Dev‑only logs by default; production clients emit **aggregated counts** (e.g., fps_percentiles, gfx_errors) or nothing—configurable.
* Expose **Prometheus** on a port (server‑side) for metrics scraping.

### 7) What to instrument (initial)

* **Tick**: `tick_ms`, `systems_active`, `systems_ms{system=...}`.
* **Networking**: `snapshot_bytes`, `delta_bytes`, `clients_connected`, `interest_entities`.
* **Projectiles**: `projectile_integrate_ms`, `projectile_hits_total`, `hit_kind{npc|player|destructible}`.
* **Destructibles**: `carve_requests_total`, `chunks_dirty`, `chunks_meshed_total`, `mesh_ms`, `collider_ms`, `queue_len`.
* **GPU upload** (client): `vb_bytes`, `ib_bytes`, `upload_ms`.
* **Errors**: counters by class (`ecs_panic_total`, `gpu_upload_fail_total`, etc.).

---

## Proposed stack (vendor‑neutral & simple)

* **`tracing` + `tracing-subscriber`** for structured logs & spans.
* **`tracing-opentelemetry`** to export spans to OTLP (OpenTelemetry Collector).
* **`metrics` crate** with **`metrics-exporter-prometheus`** (server) for /metrics.
* (Optional) **Sentry** for crash/panic reporting on both server & client.

This keeps you vendor‑agnostic: Prometheus/Grafana for metrics, Loki/ELK for logs (JSON), Tempo/Jaeger for traces.

---

# GitHub Issue

**Title:** #96 — Production‑Grade Telemetry (Logging, Metrics, Tracing)

**Labels:** observability, infra, performance, tech‑debt

**Depends on:** #95 (ECS/server‑authoritative) — can start in parallel with Phase 0/1

---

## Intent

Replace ad‑hoc prints/macros with a consistent, production‑ready observability stack across **server**, **client**, and **jobs**: structured logging (`tracing`), metrics (Prometheus), and distributed tracing (OpenTelemetry).

## Non‑Goals (for this issue)

* Full cloud deployment, long‑term retention, or vendor‑specific pipelines.
* Deep anti‑cheat analytics or BI. This is runtime telemetry for operability & debugging.

---

## Deliverables

1. **Global telemetry bootstrap** for server and client (feature‑configurable).
2. **Structured logs** (JSON) with fields & correlation IDs.
3. **Metrics endpoint** (`/metrics`) on the server.
4. **Traces** exported via OTLP with span sampling.
5. **Initial instrumentation** of hot paths (tick, projectiles, destructibles, uploads).
6. **Docs**: field conventions, sampling policies, and “what to look at” runbook.

---

## Repo‑Aware Plan

### A) Crate wiring & bootstraps

* **Add dependencies**

  * `server_core`: `tracing`, `tracing-subscriber`, `tracing-opentelemetry`, `opentelemetry`, `metrics`, `metrics-exporter-prometheus`, `once_cell`.
  * `client_core`: `tracing`, `tracing-subscriber` (console pretty in dev), optional `sentry`.
  * `render_wgpu`: depend only on `tracing` (no opentelemetry/metrics here—client_core does emission); keep upload‑side counters behind a trait.

* **Boot files**

  * `crates/server_core/src/telemetry.rs`

    ```rust
    pub struct TelemetryGuard { /* drop = flush */ }

    pub fn init_telemetry(cfg: &TelemetryCfg) -> anyhow::Result<TelemetryGuard> {
        // 1) metrics: start Prometheus exporter at cfg.metrics_addr
        // 2) tracing: set global subscriber: JSON layer + EnvFilter
        // 3) opentelemetry: build OTLP pipeline (if cfg.otlp_endpoint set)
        // 4) return guard that shuts down OTLP on drop
    }
    ```
  * `crates/client_core/src/telemetry.rs`

    ```rust
    pub fn init_client_telemetry(dev_pretty: bool) {
        // pretty console in dev; JSON when prod, but usually minimal/no OTLP
    }
    ```

* **Config**

  * `crates/data_runtime/src/configs/telemetry.rs`:

    ```rust
    pub struct TelemetryCfg {
        pub log_level: String,          // e.g., "info,render_wgpu=warn,net_core=debug"
        pub json_logs: bool,
        pub metrics_addr: Option<String>,   // "127.0.0.1:9000"
        pub otlp_endpoint: Option<String>,  // "http://localhost:4317"
        pub trace_sample: f64,              // 0.0..1.0
        pub enable_client: bool,
    }
    ```
  * Merge with env overrides: `LOG_LEVEL`, `OTLP_ENDPOINT`, `METRICS_ADDR`, etc.

### B) Replace ad‑hoc logs with `tracing`

* **Remove** `destruct_log!` and migrate to:

  ```rust
  tracing::info!(target="destruct",
      did=%did.0, zone=%zone_id, impact_id=%impact_id, cx=%cx, cy=%cy, cz=%cz,
      "carve request queued");
  ```

* **Levels**:

  * `error` = data loss, crash.
  * `warn`  = degraded behavior (skipped mesh upload).
  * `info`  = lifecycle (proxy spawned), rare.
  * `debug` = dev‑only details; gated by filter.
  * `trace` = extremely verbose (avoid in hot loops).

* **Add spans** with `#[instrument]` for key server systems (later split by #95):

  * `ProjectileIntegrateSystem::run`
  * `DestructibleRaycastSystem::run`
  * `VoxelCarveSystem::run`
  * `GreedyMeshSystem::mesh_chunk` (include `did`, `chunk`)
  * `ColliderRebuildSystem::build_chunk`

### C) Metrics (server)

* **Exporter**: start Prometheus on `metrics_addr` (if set).

* **Initial signals** (namespaces dot‑separated; labels in quotes):

  * `tick.ms` (histogram)
  * `systems.ms{system="projectile|carve|mesh|collider"}`
  * `voxel.queue_len` (gauge)
  * `voxel.chunks_dirty_total` (counter)
  * `voxel.chunks_meshed_total` (counter)
  * `voxel.mesh.ms` (histogram)
  * `voxel.collider.ms` (histogram)
  * `projectile.hits_total{kind="npc|player|destructible"}`
  * `net.snapshot.bytes_total`, `net.delta.bytes_total`, `net.clients{state="connected|disconnected"}`

* **Code pattern**:

  ```rust
  use metrics::{counter, gauge, histogram};
  histogram!("voxel.mesh.ms", elapsed_ms, "zone" => zone_id.to_string());
  counter!("voxel.chunks_meshed_total", 1, "zone" => zone_id.to_string());
  gauge!("voxel.queue_len", queue_len as f64, "zone" => zone_id.to_string());
  ```

> **Rule:** Do **not** put `player_id` or `entity_id` in metric labels. Use logs/traces for that.

### D) Tracing (OTLP)

* Add `tracing-opentelemetry` layer only when `otlp_endpoint` is set.
* Head sampling in code (`trace_sample`), tail sampling can be configured in the OpenTelemetry Collector later.
* Span fields: `zone`, `shard`, `did`, `chunk`, `impact_id`, `owner_team`, `seed`.

### E) Client (minimal)

* In dev builds: pretty `tracing-subscriber`.
* In prod: by default, **no remote exporting**; keep an option to send **breadcrumbs** (low‑rate counters) or Sentry panics if we choose.

### F) Dashboards & runbook

* Add `docs/telemetry/README.md`:

  * Field conventions (snake_case keys).
  * Sampling guidance (1–5% on heavy spans).
  * Alert suggestions:

    * `tick.ms p99 > 33ms (for 60Hz)` for 5m.
    * `voxel.queue_len > 500` for 1m.
    * `voxel.mesh.ms p95 > 20ms` for 10m.
    * `net.snapshot.bytes_total rate` spikes per shard.

---

## File changes checklist

* `crates/server_core/src/telemetry.rs` (new)
* `crates/client_core/src/telemetry.rs` (new)
* `crates/data_runtime/src/configs/telemetry.rs` (new)
* Wire `server_core::main()` (or the server bootstrap) to call `init_telemetry`.
* Replace calls in `render_wgpu/src/gfx/renderer/update.rs` from `log::info!` / `destruct_log!` to `tracing::info!` with structured fields (only where logging remains after #95).
* Add `metrics` in server systems (as they move out of renderer per #95).

---

## Acceptance Criteria

* Running the server logs a one‑line **effective telemetry config** (level, endpoints, sample rate).
* `curl localhost:$METRICS/metrics` shows the names above changing under load.
* Traces appear in your OTLP sink (Jaeger/Tempo) with spans for: projectile integrate, destructible raycast, carve, chunk mesh, collider; fields include zone and did.
* No high‑rate `info` logs in hot paths; `clippy -D warnings` clean.

---

## Stretch (separate small issues you can spin out later)

* **Sentry crash reporting** for server & client.
* **Log sampling** of noisy warnings (rate‑limit wrapper macro).
* **A/B runtime flags** (feature flags) persisted in telemetry so you can correlate perf with toggles.
* **ClickHouse or Loki** for log retention & fast filters.

---

If you want, I can also supply ready‑to‑paste code for `server_core::telemetry::init_telemetry()` (subscriber layers + Prometheus exporter bootstrap).
