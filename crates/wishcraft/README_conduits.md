# “Conduits,” Explained: A Line‑by‑Line Tour of a Real Entry

You’re about to see one line of configuration turn into a lot of power.
In *Ruins of Atlantis*, a **Conduit** is the bridge between a player’s *wish* (“what I want to happen”) and the real systems that can make it happen (code, content, maps, services). Think of it as the “authorized plug” your wish uses to reach the right machinery.

Below is a single **Conduit descriptor** written in a friendly, YAML‑like format. We’ll walk through **every line** in plain English, why it exists, and what you might change.

```yaml
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

---

## `id: "openai.codex.v2025.plan"`

**What it is:** The *stable* machine name for this conduit.
**Think of it as:** A part number. It never changes unless you ship a new version.
**Why we need it:** Wishes refer to conduits by `id`. The registry, logs, and policies all key off this value.
**How to read it:**

* `openai.codex` — the family/vendor.
* `v2025` — version or era tag (so you can evolve safely).
* `plan` — what this conduit *does* (here: generate a plan, not apply code).

> **Tip:** Treat `id` like a URL slug: short, unique, memorable, and versioned.

---

## `label: "OpenAI Codex (Plan Builder)"`

**What it is:** The human‑readable name you show in the UI.
**Why we need it:** Players and designers shouldn’t have to parse IDs; this is the friendly face.

---

## `disposition: Literalist`

**What it is:** The conduit’s *interpretation style*.
**Why we need it:** Different conduits interpret instructions differently, and that changes risk.

* **Literalist** → follows the exact wording; lower “creative drift.”
* (Others you might see later: *Maximizer*, *Egalitarian*, etc.)

> **In practice:** A Literalist planner sticks closely to your constraints and metrics. Great for safety, less exploratory.

---

## `domains: [Code]`

**What it is:** The area of the world this conduit can touch.
**Why we need it:** Helps linting, search, and UI grouping. Examples: `Code`, `WorldAuthoring`, `Logistics`, `Narrative`.

---

## `scopes: [{repo: "ruinsofatlantis", paths: ["**"], actions:["plan"]}]`

**What it is:** The *allowed surface area* for this conduit.
**Why we need it:** A wish must stay within authorized boundaries.

Breakdown of the single scope object:

* `repo: "ruinsofatlantis"`
  The repository this conduit is allowed to read when planning.

* `paths: ["**"]`
  A glob pattern list. `"**"` means “any file/folder.” You could narrow this later, e.g. `["crates/**", "!crates/render_wgpu/**"]`.

* `actions: ["plan"]`
  The verbs this conduit is allowed to perform in this scope. Here it can **plan** only (no editing, no PRs).
  Other examples you might add in other conduits: `["diff","open_pr","comment"]`.

> **Safety note:** `scopes` are your first line of defense. Narrow them as you grow.

---

## `cost_profile: { tokens_per_call: "~3k", rate_per_min: 20 }`

**What it is:** A rough “price tag” per call.
**Why we need it:** Lets players/designers budget and helps the system cap usage.

* `tokens_per_call: "~3k"` — Average prompt+response size. Tokens are bite‑sized chunks of text AIs count. The “~” means *approximate*.
* `rate_per_min: 20` — Throttle: at most 20 calls per minute.

> **In the UI:** We can show projected cost and warn when a wish will exceed capacity.

---

## `risk_class: Medium`

**What it is:** The *inherent risk* of using this conduit, feeding into Heat.
**Why we need it:** Not all conduits are equally risky; planning is safer than patching.

* `Low` → deterministic, reversible, contained.
* **Medium** → some uncertainty or scope for misinterpretation.
* `High` → could cause broad or hard‑to‑undo changes.

---

## `determinism: Stochastic`

**What it is:** Whether repeating the same input yields the same output.
**Why we need it:** Shadow‑Runs are easier to trust when outputs are deterministic.

* **Stochastic** → you may see different plans each time (typical for modern AI).
* `Deterministic` → same input → same output (great for reproducibility).

> **Effect:** Stochastic conduits may carry a small Heat uptick and stronger audit needs.

---

## `latency_class: Short`

**What it is:** A rough expectation of response time.
**Why we need it:** Sets player expectations and drives UI timeouts.

* `Instant` (<250ms)
* **Short** (seconds)
* `Long` (tens of seconds or minutes)

---

## `permissions: ["code.plan"]`

**What it is:** Named *capability flags* this conduit requires to run.
**Why we need it:** Wishes must hold the right permission scopes; missing ones fail fast.

* `code.plan` — read code, propose a plan, no writes.

> **Analogy:** Permissions are the keys; scopes are the doors they unlock.

---

## `limits: { per_wish_calls: 10, per_day_calls: 200 }`

**What it is:** Hard ceilings to prevent abuse or accidental storms of requests.

* `per_wish_calls: 10` — This wish can call the planner up to 10 times.
* `per_day_calls: 200` — Across all wishes, cap daily calls at 200.

> **Design lever:** Raise or lower these without code changes to tune the meta.

---

## `audit_fields: ["prompt_hash","model","tokens_used"]`

**What it is:** The *must‑log* fields for the Wish Ledger.
**Why we need it:** Accountability and rollback.

* `prompt_hash` — A fingerprint of what we sent (keeps secrets private but proves provenance).
* `model` — Which model/version ran (useful when models change over time).
* `tokens_used` — Resource accounting; helps cost and Heat analysis.

> **Result:** After a commit, you can always answer “what ran, on which model, and how much did it consume?”

---

# How this plays out in the wish loop

1. **Binding:** A player writes a wish and chooses this conduit for the “plan” step.
2. **Shadow‑Run:** We invoke this conduit in *simulation mode*. It returns a *proposed plan* and resource estimate; no real code changes happen.
3. **Adjudication:** Court checks clarity, safety, and that:

   * `scopes` cover the repo and paths referenced by the plan,
   * `permissions` include `code.plan`,
   * `limits` won’t be exceeded.
4. **Commit:** If approved, we call the same conduit in *commit mode* (for planning, that may just mean “finalize plan artifact”). We log `audit_fields` to the **Wish Ledger**.
5. **Heat:** Because it’s `risk_class: Medium` and `determinism: Stochastic`, the Heat model applies a modest multiplier.

---

# When would you change these values?

* **Narrow the blast radius:** Tighten `paths` from `"**"` to `["crates/**","!crates/render_wgpu/**"]`.
* **Reduce drift:** Keep `disposition: Literalist`, add more invariants to the wish, or prefer a deterministic planning conduit if available.
* **Control spend:** Lower `per_wish_calls` or `rate_per_min`.
* **Increase reliability:** If you later wrap planning in a deterministic wrapper, flip `determinism` to `Deterministic` and consider lowering `risk_class` to `Low`.

---

# A concrete example (beginner‑friendly)

> **Wish:** “Draft a 7‑day plan to reduce build times by 20% without changing public APIs.”

* The wish selects **Conduit:** `openai.codex.v2025.plan`.
* **Shadow‑Run** returns a plan like: “enable incremental caching, prune debug symbols in nightly builds, parallelize shader compilation,” with estimates.
* Court sees:

  * Repo = `ruinsofatlantis` ✔
  * Action = `plan` (no writes) ✔
  * 6 calls projected (under limit 10) ✔
  * Medium risk with Stochastic determinism → Heat + small bump ✔
* **Commit** stores the plan artifact and logs `prompt_hash`, `model`, `tokens_used`. No code is changed yet; a separate conduit (e.g., `code.apply_pr`) would handle that in another step.

---

# TL;DR (for the GDD)

* A **Conduit** is a permissioned interface that makes a piece of a wish possible.
* This descriptor says: *“Use OpenAI Codex to **plan** code changes for the Ruins of Atlantis repo, safely and within rate/usage limits, log the important bits, and expect short, somewhat variable responses.”*
* Every field is there to answer one of four questions:

  1. **What is this?** (`id`, `label`, `domains`, `disposition`)
  2. **Where may it act?** (`scopes`, `permissions`)
  3. **What will it cost / how fast is it?** (`cost_profile`, `latency_class`, `limits`)
  4. **How risky is it / how do we audit it?** (`risk_class`, `determinism`, `audit_fields`)

If you hand this entry to the registry, the system knows exactly how to bind, simulate, review, execute, and audit this conduit when a wish calls for planning code work.
