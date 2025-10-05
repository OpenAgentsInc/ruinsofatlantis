# Network & Replication — 2025-10-04

Snapshot scaffolding
- Traits: `SnapshotEncode`/`SnapshotDecode` with short-read checks (crates/net_core/src/snapshot.rs:7,12, decode helpers).
- Message types: `ChunkMeshDelta`, `DestructibleInstance`, `BossStatusMsg` with encode/decode tests (evidence/net-encode-decode.txt, evidence/net-entities-terms.txt).
- Client usage: `client_core::replication` decodes and feeds uploads (crates/client_core/src/replication.rs:33).

Channels and backpressure
- Local loop uses unbounded `std::sync::mpsc` (crates/net_core/src/channel.rs:13). Risk of unbounded growth under load.

Versioning and safety
- No version header/feature bits present. Decode paths bound lengths and bail on short reads; `BossStatusMsg` uses `from_utf8(...).unwrap_or_default()` (OK for scaffold, but consider strict error handling and caps).

Findings
- F-NET-003: Unbounded channel (P2 Med).
- F-NET-014: Add version header + caps (P2 Med).

Recommendations
- Switch to bounded channels with nonblocking `try_send` + drop/merge strategy, or implement a small ring buffer with size limits and metrics.
- Prepend a version byte and payload length cap to each message.
- Metrics: counters for bytes sent/received and queue depth; histograms for per-tick snapshot sizes.

