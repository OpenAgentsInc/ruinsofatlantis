# 95B — Scaffolds: client_core and net_core crates

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
- Workspace wiring
  - [ ] Ensure root `Cargo.toml` includes the new crates in `[workspace.members]` (already includes client_core; add net_core).
  - [ ] Update any `xtask` steps if they enumerate crates.
- CI
  - [ ] Ensure `cargo clippy -- -D warnings` and `cargo test` run for both crates.

Acceptance
- Workspace builds with `client_core` and `net_core` present; clippy/tests green.
