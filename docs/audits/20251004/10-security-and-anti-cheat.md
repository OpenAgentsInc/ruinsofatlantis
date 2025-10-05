# Security & Anti‑Cheat — 2025-10-04

Threat model
- Server-authoritative simulation is the goal; current renderer/client still performs local world mutations; ensure server is the sole source of truth before multiplayer.

Data & channels
- Unbounded channels can be abused to cause memory growth (crates/net_core/src/channel.rs:13).
- Snapshot decode paths largely bound reads and bail on short buffers (crates/net_core/src/snapshot.rs).

Findings
- F-NET-003: Unbounded channel backpressure (P2 Med).
- F-NET-014: Missing snapshot versioning/caps (P2 Med).
- F-SIM-009: Panics on server paths via `unwrap/expect` (P1 Med) can be exploited to crash servers.

Recommendations
- Introduce bounded queues with drop/merge strategies; cap per-tick inbound/outbound bytes.
- Add protocol version byte and strict decode errors; validate lengths and names; avoid `unwrap_or_default` hiding malformed inputs.
- Remove `unwrap` from server paths; return errors or sanitize defaults with metrics.

