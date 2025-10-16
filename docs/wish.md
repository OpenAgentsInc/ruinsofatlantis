# Wish System Overview

This document summarizes the Wish system as implemented in this repository and points to relevant code, data, and CLI entry points you can use today.

## What Is a Wish?

- A structured promise to the world, authored by players and enforced via bindings and audits.
- In code, a Wish is a schema with fields: objective, scope, invariants, budget, tools (aka Conduits), plan, safety_tests, rollback, and optional tier.

## Flow (Prompt → Plan → Safe Commit)

- Intention: Author a natural-language wish.
- Binding: Add clauses and produce a machine-readable Wish Schema (YAML/JSON).
- Shadow-Run: Simulate effects; produce an Echo Report (predicted diffs).
- Adjudication: Evaluate Clarity/Safety/Reversibility.
- Commit: Apply and record to the Wish Ledger; maintain rollback anchors.
- Heat & Echo: Generate Paradox Heat and Reality Echoes based on scope/novelty.

## Code Map

- Schema, Lint, Score, Heat, Conduits traits: `crates/wishcraft/src/*`
- OpenAI/Codex conduit (ChatGPT backend via auth.json): `crates/wishcraft_openai/src/*`
- CLI (lint, list conduits, plan): `xtask/src/main.rs`
- Registry (available Conduits): `data/conduits/registry.yaml`
- Example Wish: `data/wishes/wishcraft-docs.yaml`

## Conduits (Wish Enablement)

- Conduits are sanctioned interfaces a wish may use (code planner, patch applier, world placement, etc.).
- Registry entries define id, scope, risk class, determinism, limits, and required audit fields.
- The OpenAI planning conduit id is `openai.codex.v2025.plan`.

## Auth (ChatGPT via Codex)

- Credentials live at `~/.codex/auth.json` (default `CODEX_HOME`).
- We read `access_token`, `refresh_token`, and `chatgpt-account-id` (or decode from `id_token`).
- Requests target ChatGPT backend under `https://chatgpt.com/backend-api/codex` with streaming SSE.

## CLI

- Lint a wish file:
  - `cargo xtask wish lint data/wishes/wishcraft-docs.yaml`

- List available conduits (from registry):
  - `cargo xtask wish conduits list`

- Build a plan using the OpenAI planning conduit (ChatGPT backend):
  - `cargo run -p xtask -- wish codex plan --file data/wishes/wishcraft-docs.yaml --live --out /tmp/wish-docs.plan.json`
  - Uses credentials from `~/.codex/auth.json`. No API key needed.

## Tuning & Telemetry

- Heat estimation: `crates/wishcraft/src/heat.rs` (tier and conduit multipliers).
- Linting: `crates/wishcraft/src/lint.rs` (required fields, conduit registry checks).
- Scores: `crates/wishcraft/src/score.rs`.

## Related GDD Docs

- Mechanics: `docs/gdd/02-mechanics/wishcrafting.md`
- Technical schema/flow: `docs/gdd/11-technical/wishcrafting-schema-flow.md`
- Conduits spec: `docs/gdd/11-technical/wishcrafting-conduits.md`

