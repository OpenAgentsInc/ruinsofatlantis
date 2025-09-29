# Ruins of Atlantis — Codebase Audit (2025-09-29)

Author: External engineering audit (MMO client/server & engines)

Scope
- Read-only review of repository structure, crates, tests, tools, and data.
- Focus on organization, maintainability, testability, determinism, and CI/dev‑ex.
- No new features; only structural and hygiene recommendations.

Snapshot
- Workspace: modular crates for renderer (`render_wgpu`), sim (`sim_core`), data (`data_runtime`), platform (`platform_winit`), HUD, ECS, and tools. Root `src/` keeps a thin app shell.
- Tests: healthy integration tests in `tests/` covering sim/data, plus unit tests across several crates (renderer CPU helpers, HUD, collision). Gaps exist in renderer orchestration and data packing.
- Docs: strong design notes and SRD docs; `src/README.md` documents app shell and controls.
- Build tooling: `xtask` provides `ci`, `schema-check`, and pack builders. Policies in `AGENTS.md` are clear and appropriate.

Key Strengths
- Clear modular boundaries between platform, renderer, simulation, data, and tools.
- Deterministic sim core with seeded RNG and tests covering key rules paths.
- Data schemas and `xtask schema-check` reduce content drift risk.
- Renderer CPU‑side math/utilities already have unit tests; WGSL organized under one crate.

Top Risks
1) Renderer orchestration complexity and file size in `crates/render_wgpu/src/gfx/renderer/*` — long, stateful code with mixed responsibilities increases change risk and hinders testing.
2) Stringly‑typed IDs and fallback heuristics in `sim_core` create hidden coupling to file layout and spelling; type safety and explicit indices are needed for scale.
3) Incomplete schema coverage for spells/classes (serde‑only), no golden packs validation, and no stability guarantees on content hashes.
4) Tooling gaps: No dependency policy checks (e.g., `cargo deny`), no WGSL validation in CI, and no perf smoke budgets encoded.
5) Asset path discovery and Draco handling spread across crates; risk of divergence and inconsistent defaults.

High‑Impact Recommendations (90‑day horizon)
- Renderer refactor into State/Resources/Passes with a minimal frame graph and explicit resource lifetimes; add orchestration tests and CPU‑only hashing tests for buffers and culling lists.
- Introduce typed handles for abilities/spells/classes with a compile‑time key space and content‑hash driven indices; remove string fallback heuristics.
- Expand schemas to cover spells/classes/monsters; gate `packs/` on schema+golden tests; track pack versions and content hashes in artifacts.
- Strengthen CI: `xtask ci` to run Naga WGSL validation, `cargo deny`, and headless renderer CPU tests; add perf smoke time budgets for critical subsystems.
- Consolidate asset path policy into `shared/assets` and ensure a single preference order (workspace vs crate‑local) with tests.

Deliverables
- Detailed per‑area audits with prioritized backlogs: renderer, sim, data/runtime, platform/tools, CI/dev‑ex, and security/licensing.

