# codex-rs (vendored) — Structure and Wishcraft Integration Notes

This repo vendors a snapshot of the `openai/codex` Rust workspace under `third_party/openai-codex/codex-rs` for reference and optional future reuse. We do not compile it in our workspace by default.

Path in this repo
- `third_party/openai-codex/codex-rs/` (copied without `.git/` and `target/`)

Why vendored (now)
- Discoverability: keep a stable reference of the upstream Rust workspace without a network dependency.
- Evaluation: identify modules we could safely reuse as libraries later (e.g., provider config, retry/backoff, sandbox policies).
- Isolation: avoid pulling the entire Codex dependency graph into our build. No workspace linkage is added.

Upstream workspace (high-level)
- Workspace root: `codex-rs/Cargo.toml`
- Members (abridged):
  - `cli/` — Terminal UX and command plumbing for Codex.
  - `core/` — Core agent logic (planning/execution orchestration, state, policies).
  - `exec/`, `execpolicy/` — Execution workers and policy enforcement.
  - `backend-client/`, `app-server/` — Local server/client pieces.
  - `responses-api-proxy/` — HTTP proxy/thin wrapper for OpenAI Responses API.
  - `git-tooling/`, `apply-patch/`, `git-apply/` — Code application helpers.
  - `mcp-*` — Model Context Protocol client/server/types.
  - `tui/` — TUI rendering and input loop.
  - `common/`, `utils/*` — Shared helpers.

Notes on architecture (relevant to us)
- Codex is an agentic CLI with many concerns we do not embed (sandboxing, MCP orchestration, TUI, etc.).
- Planning and model access flow through OpenAI endpoints (Responses API increasingly the default), matching our Conduits design.
- Several crates (e.g., `responses-api-proxy`, `backend-client`) may contain shapes/utilities we can reuse to avoid drift.

How we connect from RoA Wishcraft

Current integration (recommended baseline)
- Use our thin `wishcraft_openai` plugin:
  - `crates/wishcraft_openai::conduit::OpenAIConduit` implements `wishcraft::conduit::ConduitExec`.
  - ShadowRun returns a safe stub plan; Commit posts to `/v1/responses` using `reqwest`.
  - This keeps `wishcraft` (core) vendor‑agnostic and the dependency surface minimal.

Optional deeper integration (later)
1) Reuse specific codex-rs modules as libraries
   - Identify library‑grade crates (e.g., `responses-api-proxy`, provider config, retry helpers).
   - Add as a path dependency (e.g., `wishcraft_openai = { path = "../../third_party/openai-codex/codex-rs/responses-api-proxy" }`) under a feature flag.
   - Caveat: codex-rs crates may assume the wider workspace; prefer vendoring small files if the dependency graph grows too large.

2) Map Codex planning to Wishcraft Conduits
   - Conduit: `openai.codex.v2025.plan` → `OpenAIConduit::exec(PlanInput)`
   - Wishcraft Wish → PlanInput mapping:
     - `objective` → PlanInput.objective
     - `tools/scopes` → PlanInput.repo/paths (from ConduitDescriptor `scopes`)
     - `invariants` → PlanInput.invariants
   - Audit fields recorded to the Wish Ledger: `prompt_hash`, `model`, `tokens_used`.

3) Tier/Heat integration
   - Conduit RiskClass/Determinism feed `estimate_heat_with_conduits()` already.
   - Codex streaming/latency can inform `latency_class` and budget estimates.

Policy and safety
- Keep Codex code outside our Cargo workspace to avoid accidental coupling.
- If we decide to depend on a codex-rs crate, gate it behind a feature (e.g., `openai-codex`) and keep a pure‑HTTP fallback.
- Add THIRD_PARTY_NOTICES if we vendor individual files.

Dev notes
- Upstream commit: run `git -C /Users/christopherdavid/code/codex-openai rev-parse HEAD` to record version when updating.
- To update vendor: `rsync -a --exclude='.git' --exclude='target' --exclude='node_modules' <path>/codex-rs/ third_party/openai-codex/codex-rs/`.

Next steps (optional)
- Evaluate `codex-rs/responses-api-proxy` for JSON shapes and retry/backoff logic.
- Add an adapter feature in `wishcraft_openai` that swaps our `client.rs` for the proxy if available.
- Add a small bridge to push Conduit audit fields into our Wish Ledger automatically on Commit.

