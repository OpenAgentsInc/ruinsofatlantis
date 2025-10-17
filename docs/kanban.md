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

3) Saved views
   - Board: `Status` columns, group by `Area`, filter `is:open`
   - Table: sort by `Priority` then `Size`
   - Bugs: filter `Type=Bug`
   - Release: filter by `Milestone=next`
   - My Work: filter `assignee:@me is:open`

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
- PR opened and linked to issue (via `Fixes #123`) → `Status=In Progress`.
- PR ready for review → `Status=Review`.
- PR merged or issue closed → `Status=Done`.
- Label changes `type:*` and `area:*` → sync to `Type` and `Area` fields.
- Add `blocked` label → set `Blocked=true`.

Note: We can use `gh` CLI in Actions or a maintained action such as `leonsteinhaeuser/project-beta-automations` for field updates. Keep logic minimal and observable in logs.

## Conventions & Policies

- Definition of Ready
  - Clear problem statement, acceptance criteria, labels (`area:*`, `type:*`, `prio:P*`), estimate (`Size`).

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
```

