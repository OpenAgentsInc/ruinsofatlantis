# Kanban & Project Management Plan

This document outlines our options for a Kanban board on GitHub and a concrete, lightweight plan tailored to this repo’s existing policies (labels, PR flow, CI). We start with native GitHub tools and document optional integrations.

## Options Overview

- GitHub Projects (new) — Recommended
  - Flexible table/board/timeline views, custom fields, iterations, saved views, and automation via GitHub Actions/`gh` CLI. Works at repo or org scope; integrates with issues/PRs.
- GitHub Project Boards (classic)
  - Simple columns + basic auto‑move. Considered legacy; lacks custom fields/slicing. Avoid for new setups unless simplicity is the only goal.
- External integrations (optional)
  - Zenhub (deep GitHub integration), Linear, Jira, Trello/Notion (with GitHub links), Azure Boards, ClickUp.
  - Self‑hosted OSS: Kanboard, Wekan, Plane. Use only if requirements exceed GitHub Projects.

Recommendation: Use GitHub Projects (new) at the repo level to keep everything close to code, labels, and PRs, with the option to lift to an org‑level Project later if cross‑repo planning is needed.

## Portfolio & Scope Strategy

Short version: keep the active delivery board laser‑focused on the current wave (e.g., VOOX ~2 months). Put longer‑horizon ideas (e.g., voxel destruction ECS follow‑ups) into a Future/Incubator space so they don’t add noise or implied commitment.

Two workable patterns:

- Single Project with a `Wave` field — Recommended if you prefer one board
  - Add single‑select `Wave`: VOOX, Future, Deferred, Unassigned
  - Saved views: `VOOX Board` (filter `Wave=VOOX`), `Future` (filter `Wave=Future`)
  - New issues default to `Wave=Unassigned`; triage moves them to `VOOX` or `Future`

- Two Projects (split by time horizon)
  - Project 10: “VOOX — Current Wave” (2‑month scope only)
  - Project X: “Incubator — Future/Exploratory” (ideas/tech spikes, no dates)
  - Items can live in both, but avoid dual‑tracking unless there’s a reason (e.g., spike in Incubator + delivery slice in VOOX)

Recommendation for this repo: Start with the single‑project pattern and add `Wave`. If the volume grows or you need different permissions/cadence, lift `Future` into its own org‑level Project later.

### What belongs where

- VOOX (current wave)
  - Deliverables targeted to the next 1–2 releases
  - Issues with clear acceptance criteria and owners; linked to milestones
  - High‑confidence tech debt that blocks wave goals

- Future/Incubator
  - Longer‑horizon features (e.g., advanced voxel destruction ECS work)
  - Exploratory spikes, research, and larger refactors without dates
  - Ideas awaiting design/feasibility or dependency unblocking

- Out of any Project (for now)
  - Draft notes, untriaged ideas not ready for discussion
  - Parking‑lot items without a problem statement — keep them in `docs/issues/` until ready

### Move Criteria (VOOX ↔ Future)

- Move to VOOX when
  - It has a crisp scope + acceptance criteria, an owner, and fits the 2‑month window
  - Dependencies are known and within team control

- Move to Future when
  - It exceeds the 2‑month window or is still research/uncertain
  - It is not tied to a near‑term milestone or budget

### Triage & Cadence

- Weekly triage
  - Inbox: new issues (Wave=Unassigned) → assign Wave, Priority, Type, Area; add to Project
  - Pull at most what fits WIP and the 2‑month window into VOOX
  - Move remainder to Future; add minimal notes/next proof step

- Monthly wave review
  - Reconfirm scope for VOOX; promote ready Future items; demote out‑of‑scope items

## Recommended Workflow

- Status flow (columns): Backlog → Ready → In Progress → Review → Done
- Labels (reuse existing): `area:*`, `type:*`, `prio:P0..P3`, `perf`, `determinism`, `schema-change`, `docs-needed`.
- Estimation: Fibonacci sizing (1, 2, 3, 5, 8). Reserve `P0` for expedite.
- Ownership: One assignee per in‑progress item; reviewers on linked PRs.
- WIP limits: 2 in `In Progress` per engineer; 3 in `Review` per team.

## Project Setup (GitHub Projects)

1) Create the Project (repo‑scoped)
   - UI: GitHub → Projects → New project → Link to this repo.
   - Or CLI: `gh project create "Engineering Kanban" --public=false --source .`

2) Define custom fields
   - `Status` (built‑in): Backlog, Ready, In Progress, Review, Done
   - `Priority` (single‑select): P0, P1, P2, P3
   - `Type` (single‑select): Bug, Feature, TechDebt, Docs
   - `Area` (single‑select): gfx, sim, data, hud, ecs, client, server, tools, docs
   - `Size` (number)
   - `Blocked` (checkbox) with `Blocked reason` (text)
   - `Iteration` (Projects iterations) for sprints/timeboxes (optional)
   - `Wave` (single‑select): VOOX, Future, Deferred, Unassigned

3) Saved views
   - Board: `Status` columns, group by `Area`, filter `is:open`
   - Table: sort by `Priority` then `Size`
   - Bugs: filter `Type=Bug`
   - Release: filter by `Milestone=next`
   - My Work: filter `assignee:@me is:open`
   - VOOX: filter `Wave=VOOX`
   - Future: filter `Wave=Future`

4) Backlog import
   - Add existing repo issues to the Project (bulk select in Issues list → `Projects` → add).
   - Default new issues to `Backlog` (automation below).

## Automations (via GitHub Actions)

Add a single workflow that keeps status in sync between issues and PRs. Below are snippets to include when we decide to land automation. We will map labels to project fields and auto‑move items on events.

Auto‑add issues to the Project and set defaults:

```yaml
# .github/workflows/project-automation.yml
name: Project Automation
on:
  issues:
    types: [opened, labeled, unlabeled, closed, reopened]
  pull_request_target:
    types: [opened, ready_for_review, converted_to_draft, closed, reopened, labeled, unlabeled]
permissions:
  contents: read
  issues: write
  pull-requests: write
  projects: write
jobs:
  sync:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/github-script@v7
        env:
          PROJECT_NUMBER: 1               # set after creating the Project
          ORGANIZATION: ${{ github.repository_owner }}
        with:
          script: |
            // Pseudocode: add item to project, set Status, Priority from labels, etc.
            // Recommend using a maintained action (e.g., leonsteinhaeuser project actions)
            // or the GraphQL API to set fields deterministically.
```

Suggested transitions:
- Issue opened → add to Project with `Status=Backlog`; map `prio:P0..P3` → `Priority`.
- If `Wave` unset → set `Wave=Unassigned`; triage promotes to `VOOX` or moves to `Future`.
- PR opened and linked to issue (via `Fixes #123`) → `Status=In Progress`.
- PR ready for review → `Status=Review`.
- PR merged or issue closed → `Status=Done`.
- Label changes `type:*` and `area:*` → sync to `Type` and `Area` fields.
- Add `blocked` label → set `Blocked=true`.

Note: We can use `gh` CLI in Actions or a maintained action such as `leonsteinhaeuser/project-beta-automations` for field updates. Keep logic minimal and observable in logs.

## Conventions & Policies

- Definition of Ready
  - Clear problem statement, acceptance criteria, labels (`area:*`, `type:*`, `prio:P*`), estimate (`Size`).
  - Assigned `Wave` (VOOX for current wave); otherwise keep in `Future` or `Unassigned` views.

- Definition of Done
  - Code merged, CI green, tests/docs updated, perf note if GPU cost ≥ 0.5 ms, labels accurate.

- Cadence
  - Weekly backlog grooming; daily standup references Project views; track blocked items.

- PR & Branch policy
  - Branch name and PR style per repo policy; link issues in PR bodies (`Fixes #id`).

- WIP limits & focus
  - Respect WIP limits; split oversized work; use `blocked` when waiting.

## Metrics (lightweight)

- Cycle time (In Progress → Done), lead time (Backlog → Done), throughput per week.
- Use Projects’ built‑in charts where available; otherwise export to CSV and compute.
- Query ideas:
  - `is:issue is:open label:type:Bug sort:updated-desc`
  - `project:"Engineering Kanban" status:"Review" assignee:@me`

## Alternatives & When to Revisit

- Move to org‑level Project if we add more repos or need shared roadmaps.
- Consider Linear or Jira if we need advanced dependencies/approvals or cross‑team rollups.
- If we only need a visual board without fields/metrics, Classic Boards are acceptable but limiting.

## Next Actions (Checklist)

- [ ] Decide scope (repo vs org Project) and name
- [ ] Create Project and fields; add saved views
- [ ] Bulk add existing issues; tag `prio:*`, `type:*`, `area:*`
- [ ] Land automation workflow (status sync, label→field mapping)
- [ ] Announce conventions (this doc) and start date
- [ ] Review after 2 weeks; adjust fields, views, WIP limits
 - [ ] Add `Wave` field and saved views (`VOOX`, `Future`); assign Waves during weekly triage
 - [ ] Move non‑VOOX items (e.g., voxel destruction ECS expansions) to `Wave=Future` or to a separate “Incubator” Project if preferred

---

Appendix: `gh` CLI helpers

```bash
# Create project (repo scoped)
gh project create "Engineering Kanban" --source . --public=false

# List fields and items
gh project field-list "Engineering Kanban"
gh project item-list "Engineering Kanban" --format json > project.json

# Add an issue to the project
gh issue list -s all
gh project item-add "Engineering Kanban" --url https://github.com/$REPO/issues/123

# Create `Wave` field and seed options (single project pattern)
gh project field-create $NUM --owner $OWNER --name Wave --data-type SINGLE_SELECT --single-select-options VOOX,Future,Deferred,Unassigned

# Set Wave for an item
gh project item-edit --project-id $(gh project view $NUM --owner $OWNER --format json -q .id) \
  --id $ITEM_ID --field-id $(gh project field-list $NUM --owner $OWNER --format json | jq -r '.fields[] | select(.name=="Wave").id') \
  --single-select-option-id $(gh project field-list $NUM --owner $OWNER --format json | jq -r '.fields[] | select(.name=="Wave").options[] | select(.name=="VOOX").id')
```
