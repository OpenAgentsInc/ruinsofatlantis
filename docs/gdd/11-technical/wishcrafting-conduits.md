add where relevant -- verbatim, dont condense it - Awesome—“**Conduits**” it is. Here’s a tidy, drop-in GDD spec you can hand to the codebase agent. It stays system-level (no other-profession entanglements) and aligns with the `wishcraft` crate you just added.

---

# Conduits (Wish Enablement Layer)

## Concept

**Conduits** are sanctioned interfaces that *enable* a wish to act on the world. A conduit can be an API integration (e.g., OpenAI Codex for code changes in 2025), a local tool, or any execution path that transforms intent into concrete changes.
**Rule of thumb:** *A wish without appropriate conduits is only a petition.*

## Player-Facing Vocabulary

* **Conduit** — the interface that carries a clause of your wish into action.
* **Conduit Registry** — catalog of available conduits and what they can touch.
* **Disposition** — how a conduit interprets instructions (e.g., literalist, maximizer, egalitarian).
* **Scope & Limits** — where the conduit is allowed to act and how far it can go.

(Internally, the code uses the same term: `Conduit*` types.)

---

## Role in the Wish Loop

1. **Intention** — Player authors wish text.
2. **Binding** — Player selects **Conduits** and assigns them to plan steps; sets **Scope, Invariants, Budget**.
3. **Shadow-Run** — Conduits run in **simulation/mocked** mode; system produces an **Echo Report** (predicted diffs).
4. **Adjudication** — Court checks **Clarity/Safety/Reversibility** and whether chosen conduits are permitted for the tier.
5. **Commit** — Conduits execute for real under caps/rate limits; outputs are logged to the **Wish Ledger** with a rollback anchor.
6. **Heat & Echo** — Each conduit contributes to Heat based on **risk class** and **determinism**.

---

## Data Model (Registry Entry)

```
ConduitDescriptor {
  id: String,                 // "openai.codex.v2025.plan"
  label: String,              // "OpenAI Codex (Plan Builder)"
  disposition: Disposition,   // Literalist | Maximizer | Egalitarian | ...
  domains: [Domain],          // e.g., Code, Content, Cartography, Logistics
  scopes: [ScopeRule],        // where it may act (paths, zones, repos, services)
  cost_profile: CostProfile,  // per-call costs (gold, time, tokens, rate caps)
  risk_class: RiskClass,      // Low | Medium | High (affects Heat)
  determinism: Determinism,   // Deterministic | Stochastic | Mockable
  latency_class: Latency,     // Instant | Short | Long
  permissions: [Permission],  // named capabilities it can exercise
  limits: Limits,             // per-wish/per-day ceilings
  audit_fields: [String],     // must be logged (e.g., prompt hash, diff hash)
}
```

**Notes**

* *Disposition* informs drift risk (used by scoring & UI hints).
* *ScopeRule* binds to concrete resources (e.g., repo path `game/engine/**`, zone slug `wizard_woods`).
* *RiskClass* feeds Heat calculation.
* *Determinism* toggles how Shadow-Run stubs/mocks it.

---

## Execution Interface (Code-Level)

Add a `conduit` module to `wishcraft` (neutral, pure Rust):

```rust
// wishcraft/src/conduit.rs
use serde::{Serialize, Deserialize};
use anyhow::Result;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Disposition { Literalist, Maximizer, Egalitarian }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConduitDescriptor { /* as in Data Model above */ }

pub trait ConduitRegistry {
    fn get(&self, id: &str) -> Option<ConduitDescriptor>;
    fn allow(&self, id: &str) -> bool { self.get(id).is_some() }
}

pub trait ConduitExec {
    type Input: Serialize + for<'de> Deserialize<'de>;
    type Output: Serialize + for<'de> Deserialize<'de>;

    /// Execute a conduit operation (real or simulated).
    /// `mode` controls simulation vs commit.
    fn exec(&self, conduit_id: &str, input: Self::Input, mode: ExecMode) -> Result<Self::Output>;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecMode { ShadowRun, Commit }
```

**Why:** keeps `wishcraft` vendor-agnostic; plugins (e.g., `wishcraft-genies-openai`) implement `ConduitExec`.

---

## Binding Rules

* Each **plan step** in a wish may **require** a conduit.
* Binding requires:

  * Conduit present in **Registry**.
  * Conduit **Scope** covers the targeted resource(s).
  * Conduit **Permissions** satisfy the step (e.g., “open PR”, “run CI”).
* Missing or disallowed conduits → **lint error**.
* Exceeding **Limits** → **court failure** or defer to scheduled window.

---

## Shadow-Run Requirements

* Conduits must support **`ExecMode::ShadowRun`**.
* Shadow mode **must not** mutate real systems; it returns:

  * **Predicted Diffs** (e.g., code patch summary, map deltas).
  * **Risk Flags** (P0-P3).
  * **Resource Estimates** (tokens, calls, time).
* If a conduit cannot mock, the wish must include a **Proxy Tester** (separate conduit) or it is **Macro-tier only** with heightened Heat.

---

## Commit Requirements

* Enforce **rate limits** & **caps** from the descriptor.
* Produce **Audit Fields** to the Wish Ledger (e.g., prompt hash, model, token usage, diff hash, artifact URIs).
* **Rollback expectation** must be met:

  * If the conduit can auto-revert, provide a handle/anchor.
  * If not, the wish must specify a **manual rollback plan**.

---

## Security & Secrets

* Conduits declare **permission scopes** (e.g., `repo:roa:write:pull_request`).
* Secrets (API keys, tokens) are resolved **outside** the wish payload via environment/secret store.
* **Redaction:** prompts/inputs are hashed or partially redacted in the Ledger unless `debug_secrets=true` (dev only).

---

## Heat Integration

Base Heat multiplier per conduit:

```
Heat_conduit = Base
             * RiskClassFactor
             * (1.2 if Determinism == Stochastic && ShadowRun coverage < 80% else 1.0)
             * (1.1 if Disposition == Maximizer else 1.0)
```

Design can tune factors in `configs/wishcraft.toml`.

---

## Failure Modes & Policy

* **Timeout:** auto-retry (bounded), then fail the step and pause the wish for amendment.
* **Partial Success:** ledger partials; wish remains “amendable” for 24h.
* **Circuit Breaker:** trip if 3+ failures in 10m; court auto-flags the conduit for review.

---

## Tiering

* **Micro:** Local/safe conduits only (low risk class, known scopes).
* **Meso:** Adds org-level conduits (e.g., CI/CD).
* **Macro:** Allows external world-affecting conduits (vendor APIs at scale), requires petition quorum and higher scores.

---

## Telemetry

Per conduit call, record:

* `wish_id`, `conduit_id`, `mode`
* `latency_ms`, `attempts`, `tokens_used` (if applicable)
* `result_size`, `diff_hash`
* `heat_delta`, `risk_flags`
* `rollback_anchor` (if any)

---

## Examples (descriptors)

**OpenAI Codex – Plan Builder (2025)**

```
id: "openai.codex.v2025.plan"
label: "OpenAI Codex (Plan Builder)"
disposition: Literalist
domains: [Code]
scopes: [{repo: "ruinsofatlantis", paths: ["**"], actions:["plan"]}]
cost_profile: { tokens_per_call: "~3k", rate_per_min: 20 }
risk_class: Medium
determinism: Stochastic
latency_class: Short
permissions: ["code.plan"]
limits: { per_wish_calls: 10, per_day_calls: 200 }
audit_fields: ["prompt_hash","model","tokens_used"]
```

**OpenAI Codex – Apply Patch (PR)**

```
id: "openai.codex.v2025.apply"
label: "OpenAI Codex (Apply Patch)"
disposition: Maximizer
domains: [Code]
scopes: [{repo: "ruinsofatlantis", actions:["open_pr"]}]
risk_class: High
permissions: ["code.diff","code.open_pr"]
limits: { per_wish_calls: 3 }
audit_fields: ["diff_hash","pr_url","model"]
```

**Worldsmithing – Place Entities (stub)**

```
id: "worldsmith.place"
label: "Worldsmithing Placement"
disposition: Literalist
domains: [WorldAuthoring]
scopes: [{zone: "*", actions:["place","remove"]}]
risk_class: Low
determinism: Deterministic
permissions: ["place","remove"]
limits: { per_wish_entities: 500 }
audit_fields: ["placements_count"]
```

---

## V1 Deliverables (for the agent)

1. **wishcraft crate**

   * Add `conduit.rs` with `ConduitDescriptor`, `ConduitRegistry`, `ConduitExec`, `Disposition`, `ExecMode`.
   * Add `lint` checks:

     * Every referenced `tools` entry in a Wish must resolve in `ConduitRegistry`.
     * Verify step→permission mapping (basic).
   * Add `heat` integration for `RiskClass` & `Determinism`.

2. **xtask**

   * `xtask wish conduits list` — print available descriptors (from a JSON/YAML registry file).
   * `xtask wish shadow-run` — load descriptors; run stubs when `ExecMode::ShadowRun`.

3. **Data**

   * `data/conduits/registry.yaml` — seed with *Codex (plan/apply)* and *Worldsmithing (place)* examples above.

4. **Plugin crate (optional now / recommended)**

   * `crates/wishcraft-conduits-openai` implementing `ConduitExec` for Codex.
   * Reads `OPENAI_API_KEY`, `OPENAI_MODEL` from env.

5. **Docs (GDD)**

   * Add this “Conduits” section under Wishcrafting → Systems.

---

## Acceptance Criteria

* A wish referencing `tools: ["openai.codex.v2025.plan"]` **fails lint** if that conduit is missing from the registry or out of scope.
* Shadow-Run with Codex conduit **returns a mocked Echo Report** (no external calls) unless the plugin is enabled.
* Heat increases when using a High-risk conduit; reductions are visible when switching to Low-risk ones.
* Ledger entries include the conduit’s **audit fields**.

---

If you want, I can also produce the `data/conduits/registry.yaml` starter and the `conduit.rs` file stub matching this spec.

