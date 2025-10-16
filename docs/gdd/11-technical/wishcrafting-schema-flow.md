## Wishcrafting — Schema & Flow (One‑Pager)

Purpose
- Define a lightweight, typed Wish Schema and a clear Petition → Shadow‑Run → Commit flow that aligns the in‑game Wishcrafting fantasy with a safe, auditable developer harness.

Scope (V1)
- Tier: Micro wishes only, limited to a single region.
- Outputs: machine‑readable Wish YAML/JSON, Echo Report (predicted diffs), Court scores, Ledger entry with rollback anchor.
- Guardrails: Anchor Invariants enforced; Paradox Heat estimated and capped; rollback plan required.

Data Model (Wish Schema v0.1)
- Fields (authoritative keys):
  - `title: string`
  - `objective: string` (measurable target; avoid pronouns and vague verbs)
  - `scope: { region: string, duration_days: u16 }`
  - `invariants: string[]` (world‑law constraints)
  - `budget: { chrono_sand: u16, genie_slots: u8, gold_cap: u64 }`
  - `tools: string[]` (Genie Registry IDs)
  - `plan: string[]` (concise steps; ≤ 7 items)
  - `safety_tests: string[]` (sim checks; describe metric + bound)
  - `rollback: string[]` (explicit revert actions)
  - `tier: enum { micro | meso | macro }` (derived or declared)
  - `meta?: { author_id: string, petition_id: string, created_at: iso8601 }`

Lint Rules (CI)
- Required fields present; `objective` includes concrete quantities and time bounds.
- `scope.region` is known; `duration_days` within tier limits.
- `invariants` include at least: no harm to civilians, no agency removal, no currency duplication (configurable set).
- `budget` within per‑tier maxima; non‑negative integers.
- `tools[*]` exist in Genie Registry and are allowed for the tier.
- `rollback` must restore primary affected metrics to within bounds (declared) and be feasible without new tools.

Scoring Rubric (Wish Court Bot)
- Clarity (0–100): penalize ambiguous phrases, missing quantities, pronouns; reward numeric targets and explicit bounds.
- Safety (0–100): invariant coverage, tool risk profile, simulated side‑effect magnitude, estimated Heat.
- Reversibility (0–100): rollback completeness, estimated revert cost/time, blast radius containment.
- Thresholds (example):
  - Micro ≥ (C:70, S:60, R:60)
  - Meso ≥ (C:75, S:70, R:70)
  - Macro ≥ (C:85, S:85, R:80) + petition quorum

Heat Estimate (reference)
- Formula: `Heat = Base * ScopeFactor * Novelty * Speed * RevPenalty * ClarityPenalty * ChainLen - Mitigations`
- Defaults: ScopeFactor {micro:0.5, meso:1.0, macro:2.0}; Novelty 0.8–1.5; Speed 1.0–1.4; RevPenalty 0.8–1.3; ClarityPenalty 0.8–1.4; Mitigations from cooling rituals, public mandate, genie match.

Genie Registry (concept)
- Catalog of callable systems/tools with IDs, caps, costs, and persona traits.
- Example ID format: `Weather.StormOracle`, `Logistics.ConvoyPlanner`.
- Used for lint (existence/allowlist) and for sim cost modeling.

Flow (Petition → Shadow‑Run → Commit)
1) Petition
   - Player submits Wish text; UI guides into structured fields.
   - Output: `wish.yaml` with derived `tier` and initial Heat estimate.
2) Schema Lint (blocking)
   - Run lints; compute Court draft scores; highlight fixes; require minimum Clarity.
3) Shadow‑Run
   - Execute plan against a staging snapshot (region‑scoped). Tools are stubbed or sandboxed.
   - Output: Echo Report (predicted entity/state diffs, indices deltas, risk flags) + metrics summary.
4) Review (Wish Court)
   - Automated + human rubric; optional amicus clauses add/adjust invariants; recompute scores.
5) Commit
   - Create Ledger entry: hash of `wish.yaml`, Echo Report summary, rollback instructions, anchor ID.
   - Apply staged changes transactionally; enforce caps and invariants.
6) Monitor & Amend
   - Track post‑commit metrics for N days; auto‑surface anomalies; enable amendment/rollback via the anchor.

Artifacts & Integration
- Inputs: `wish.yaml` (YAML/JSON), Genie Registry catalog, region snapshot handle.
- Outputs: Echo Report (human + machine readable), Court scores, Ledger entry.
- Telemetry: metrics for Time‑to‑First‑Impact, Overheat Rate, Rollback Frequency, Template Reuse Rate.

Security & Permissions
- All tool calls scoped to declared `tools` and tier caps; no implicit capabilities.
- Every commit logged to Ledger with author and anchor; rollbacks auditable.
- Never bind real external APIs without explicit connector approval and dry‑run coverage.

MVP Notes
- Micro‑tier only; limited tool surface; non‑destructive sim by default.
- Court Bot ships as a CLI (or xtask) that lints, scores, and produces Echo Reports from `wish.yaml`.
- UI shows Ambiguity Meter, predicted vs. actual diff cards, and Heat meter.

