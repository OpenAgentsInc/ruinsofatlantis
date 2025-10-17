# Issue Authoring in Repo

This folder holds plain‑Markdown issue specs that we can publish to GitHub via the `gh` CLI. It keeps planning close to code and supports reviewable edits via PRs before filing issues.

## File Naming

- Use short, kebab‑case filenames (e.g., `virtual-camera-cinematics.md`).
- Prefer prefixes by area when helpful (e.g., `gfx-virtual-camera.md`).

## Front Matter (simple)

Each issue file begins with a simple front matter block. Keep keys single‑line for easy parsing by our script.

```
---
title: Renderer: Virtual Camera & Cinematics System
labels: area:gfx, type:Feature, prio:P1, docs-needed
assignees: 
milestone: 
---
```

- `title`: Required. Be explicit and scoped.
- `labels`: Comma‑separated. Reuse repo labels (`area:*`, `type:*`, `prio:P*`).
- `assignees`: Optional, comma‑separated GitHub handles.
- `milestone`: Optional, exact milestone name.

Project add (optional): set env vars when publishing:
- `PROJECT_OWNER` and `PROJECT_NUMBER` to add the created issue to the GitHub Project (new Projects).

## Body Structure (recommended)

After the front matter, use these sections:

- Context
- Goals / Non‑Goals
- Acceptance Criteria
- Tasks
- Dependencies / Risks
- Notes / Design Sketch
- Links

See `_template.md` for a copy‑paste starter.

## Publish to GitHub

Prereqs: install and auth GitHub CLI (`gh auth login`).

```
# Dry run (prints parsed fields)
scripts/issue_from_doc.sh --dry-run docs/issues/virtual-camera-cinematics.md

# Create the issue in the current repo
scripts/issue_from_doc.sh docs/issues/virtual-camera-cinematics.md

# Also add to a Project (set once in your shell)
export PROJECT_OWNER="<org-or-user>"
export PROJECT_NUMBER=1
scripts/issue_from_doc.sh docs/issues/virtual-camera-cinematics.md
```

Notes:
- The script parses simple `key: value` front matter. Keep lists comma‑separated.
- Automation in CI will later map labels to Project fields and Status.

