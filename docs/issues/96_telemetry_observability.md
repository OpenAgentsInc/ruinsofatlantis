# 96 — Production‑Grade Telemetry (Logging, Metrics, Tracing)

Status: COMPLETE (initial scaffold + server metrics + docs)

Labels: observability, infra, performance
Depends on: Epic #95 (parallel‑safe)

Intent
- Establish a coherent telemetry stack: structured logs (`tracing`), low‑cardinality metrics (`metrics` + Prometheus), and hooks for traces (OTLP endpoint optional).

Scope & Outcomes
- Server bootstrap helper: `server_core::telemetry::init_telemetry(cfg)` sets `tracing-subscriber` and optional Prometheus exporter.
- Config loader: `data_runtime::configs::telemetry` reads `data/config/telemetry.toml` + env overrides.
- Client helper: `client_core::init_client_telemetry(pretty)` for dev.
- Instrumentation (initial): metrics in destructible systems (carve/mesh/collider) and projectiles (integrate/collide hit counters + ms histograms).
- Sample config committed under `data/config/telemetry.toml`.

Acceptance
- Server code can enable metrics by setting `metrics_addr` in config or `METRICS_ADDR` env; `/metrics` serves counters/histograms.
- Logs default to JSON; level overridable via `LOG_LEVEL`.
- No high‑rate logs added to hot loops; metrics used instead.

Addendum — Implementation Summary
- Added `data/config/telemetry.toml` with defaults; env overrides already supported (`LOG_LEVEL`, `JSON_LOGS`, `TRACE_SAMPLE`, `METRICS_ADDR`, `OTLP_ENDPOINT`).
- `server_core::telemetry::init_telemetry` now emits a one‑line summary of effective config on init via `tracing::info!(target="telemetry", ...)`.
- Instrumented server systems with `metrics`:
  - `voxel.carve.ms` (histogram), `voxel.carve_requests_total` (counter), `voxel.queue_len` (gauge)
  - `voxel.mesh.ms` (histogram), `voxel.chunks_meshed_total` (counter)
  - `voxel.collider.ms` (histogram), `voxel.colliders_rebuilt_total` (counter)
  - `projectile.integrate.ms` (histogram), `projectile.collide.ms` (histogram), `projectile.hits_total{kind="npc|player|destructible"}` (counters)
- Existing tests: `crates/data_runtime/tests/telemetry_cfg.rs` validates env parsing.

Notes / Next
- Hook OTLP/tracing exporter when an endpoint is configured; today only the subscriber is initialized.
- Consider calling `init_telemetry` early during native startup (guarded and idempotent) when we introduce a long‑running server.
- Add a docs/telemetry runbook (dashboards, alert suggestions) when dashboards exist.

