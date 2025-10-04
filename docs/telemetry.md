# Telemetry — Logs, Metrics, Traces (dev usage)

This project ships a lightweight, vendor‑neutral telemetry scaffold:
- Logs use `tracing` with JSON output by default (pretty in client dev).
- Metrics use the `metrics` crate with an optional Prometheus HTTP exporter.
- Tracing hooks are ready; set an OTLP endpoint to export spans later.

Quick start
- Config file: `data/config/telemetry.toml` (committed). Env vars override.
- Typical env vars:
  - `LOG_LEVEL=info,ruinsofatlantis=info,client_core=debug` — set module log levels
  - `JSON_LOGS=true|false` — switch JSON vs. plain logs (server)
  - `METRICS_ADDR=127.0.0.1:9100` — enable Prometheus exporter at `/metrics`
  - `OTLP_ENDPOINT=http://localhost:4317` — (future) export spans via OTLP
  - `TRACE_SAMPLE=0.05` — (future) head sampling for heavy spans

Where telemetry is initialized
- Server: `server_core::telemetry::init_telemetry(&cfg)` sets up `tracing` and, if configured, Prometheus exporting. It logs a one‑line summary with the effective config.
- Client: `client_core::init_client_telemetry(true)` provides pretty console logs for dev; production clients typically leave remote exporting off.

What’s instrumented today (server)
- Destructibles systems
  - `voxel.carve_requests_total` (counter), `voxel.queue_len` (gauge), `voxel.carve.ms` (histogram)
  - `voxel.chunks_meshed_total` (counter), `voxel.mesh.ms` (histogram)
  - `voxel.colliders_rebuilt_total` (counter), `voxel.collider.ms` (histogram)
- Projectiles
  - `projectile.integrate.ms`, `projectile.collide.ms` (histograms)
  - `projectile.hits_total{kind="npc|player|destructible"}` (counters)

Prometheus
- Enable the exporter by setting `metrics_addr` in `data/config/telemetry.toml` or `METRICS_ADDR` in the environment.
- Then `curl http://127.0.0.1:9100/metrics` to scrape.
- Keep labels low‑cardinality (zone, shard, system). Do not include player or entity IDs in labels.

Logs
- JSON logs by default on the server (`json_logs=true`). Use `LOG_LEVEL` to adjust filters.
- Use structured fields for context (e.g., `did`, `chunk`), but prefer metrics for hot‑loop counters.

Traces (future toggle)
- When `OTLP_ENDPOINT` is set, wire a `tracing-opentelemetry` layer to export spans. Keep sampling low (`TRACE_SAMPLE` ≤ 0.05) unless tail‑sampling is configured in your collector.

Policies
- No high‑rate logs in hot loops; prefer metrics.
- Avoid PII in logs and metric labels; use numeric IDs.

