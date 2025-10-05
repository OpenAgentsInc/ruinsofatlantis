# Observability & Ops — 2025-10-04

Status
- Server telemetry bootstrap sets up tracing and optional Prometheus exporter (crates/server_core/src/telemetry.rs:5,27).
- Metrics usage exists for boss spawns; limited elsewhere (evidence/telemetry-uses.txt).

Findings
- F-OBS-011: Missing key counters/histograms for budgets, tick time, net bytes, queue depths (P2 Low/Med depending on area).

Recommendations
- Counters: `boss.spawns_total`, `net.bytes_sent/received_total`, `replication.queue_depth`.
- Histograms: `tick.ms`, `mesh.ms`, `collider.ms`, `snapshot.size.bytes`.
- Logs: avoid high-rate logs in loops; prefer metrics; current grep shows no obvious floods (evidence/log-flood-sites.txt empty).

