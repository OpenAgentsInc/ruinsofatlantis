# 95B — Scaffolds: client_core and net_core crates

Status: COMPLETE (scaffolds landed; CI enforces clippy/tests)

Labels: infrastructure, networking
Depends on: Epic #95 (ECS/server-authoritative)

Intent
- Create scaffolding for client systems (replication, uploads) and net snapshot plumbing (local loop first).

Outcomes
- New crates compile in workspace; minimal modules exist; CI runs clippy/tests.

Repo‑aware Inventory
- `crates/client_core` exists with `input` and a basic `controller`; add structured modules for replication/upload/systems.
- `crates/net_core` does not exist; add crate for snapshot/apply/interest scaffolds.

Tasks
- Add/expand crates
  - [ ] `crates/client_core/src/{replication.rs,upload.rs,systems/mod.rs}` stubs (Rustdoc each with responsibilities).
  - [ ] `crates/net_core/src/{snapshot.rs,apply.rs,interest.rs}` stubs (define traits/messages to be filled in Phase 3).
- Crate metadata & lints
  - [ ] In `crates/client_core/src/lib.rs` and `crates/net_core/src/lib.rs`, add strict lints so agents inherit discipline:
    ```rust
    #![deny(warnings, clippy::all, clippy::pedantic)]
    #![allow(clippy::module_name_repetitions)] // as needed for module structure
    ```
- Guiding traits & TODOs
  - [ ] `net_core/src/snapshot.rs`:
    ```rust
    pub trait SnapshotEncode { fn encode(&self, out: &mut Vec<u8>); }
    pub trait SnapshotDecode: Sized { fn decode(inp: &mut &[u8]) -> anyhow::Result<Self>; }
    // Stub types: EntityHeader, ChunkMeshDelta { did, chunk, positions, normals, indices }
    ```
  - [ ] `client_core/src/upload.rs`:
    ```rust
    pub struct ChunkMeshEntry { pub positions: Vec<[f32;3]>, pub normals: Vec<[f32;3]>, pub indices: Vec<u32> }
    pub trait MeshUpload {
        fn upload_chunk_mesh(&mut self, did: u64, chunk: (u32,u32,u32), mesh: &ChunkMeshEntry);
        fn remove_chunk_mesh(&mut self, did: u64, chunk: (u32,u32,u32));
    }
    ```
- Workspace wiring
  - [ ] Ensure root `Cargo.toml` includes the new crates in `[workspace.members]` (already includes client_core; add net_core).
  - [ ] Update any `xtask` steps if they enumerate crates.
- CI
  - [ ] Ensure `cargo clippy -- -D warnings` and `cargo test` run for both crates.
  - [ ] Add minimal doc tests in both crates to ensure tests execute.

Acceptance
- Workspace builds with `client_core` and `net_core` present; clippy/tests green.

---

## Addendum — Implementation Summary (95B landed)

What was implemented:
- client_core
  - Added strict lints with targeted allows.
  - New modules: `replication` (buffer stub + test), `upload` (ChunkMeshEntry + MeshUpload trait + test), `systems` (placeholder + test).
  - Wired modules in `lib.rs`; annotated `PlayerController::new` with `#[must_use]`.
- net_core (new crate)
  - Added `snapshot.rs` with `SnapshotEncode`/`SnapshotDecode`, `EntityHeader`, and `ChunkMeshDelta` (naive encode/decode + roundtrip test).
  - Added `apply.rs` (`ReplicationApply` stub) and `interest.rs` (`InterestProvider` stub).
  - Strict lints, crate smoke test.
- Workspace
  - Added `crates/net_core` to workspace members so CI covers it.
- CI
  - Existing `xtask ci` remains green; new crates now participate in clippy/tests.
