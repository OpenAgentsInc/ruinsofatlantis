#!/usr/bin/env bash
set -euo pipefail

# Adds all open issues from the current repo to a GitHub Project (new Projects)
# and sets Status to Backlog.
#
# Required env:
#   PROJECT_OWNER  # org/user login (e.g., OpenAgentsInc)
#   PROJECT_NUMBER # project number (e.g., 10)
#
# Optional flags:
#   --all  # include closed issues as well (closes will still be set to Backlog unless adjusted)

usage() {
  cat <<USAGE
Usage: PROJECT_OWNER=<owner> PROJECT_NUMBER=<number> $0 [--all]

Adds all open issues to the specified GitHub Project and sets Status=Backlog.
USAGE
}

INCLUDE_ALL=0
if [[ "${1:-}" == "--all" ]]; then
  INCLUDE_ALL=1
fi

command -v gh >/dev/null 2>&1 || { echo "gh CLI is required" >&2; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "jq is required" >&2; exit 1; }

OWNER=${PROJECT_OWNER:-}
NUM=${PROJECT_NUMBER:-}
[[ -n "$OWNER" && -n "$NUM" ]] || { usage; exit 1; }

echo "Syncing issues to project $OWNER/$NUM ..." >&2

PROJECT_JSON=$(gh project view "$NUM" --owner "$OWNER" --format json)
PROJECT_ID=$(echo "$PROJECT_JSON" | jq -r '.id')

FIELDS_JSON=$(gh project field-list "$NUM" --owner "$OWNER" --format json)
STATUS_FIELD_ID=$(echo "$FIELDS_JSON" | jq -r '.fields[] | select(.name=="Status").id')
BACKLOG_OPTION_ID=$(echo "$FIELDS_JSON" | jq -r '.fields[] | select(.name=="Status").options[] | select(.name=="Backlog").id')

[[ -n "$PROJECT_ID" && -n "$STATUS_FIELD_ID" && -n "$BACKLOG_OPTION_ID" ]] || {
  echo "Unable to resolve project or Status/Backlog field IDs" >&2; exit 1;
}

# Current project items (issue numbers)
ITEMS_JSON=$(gh project item-list "$NUM" --owner "$OWNER" --format json)
REPO_SLUG=$(gh repo view --json nameWithOwner -q .nameWithOwner)
# Safely collect existing issue numbers for this repo only
EXISTING=$(echo "$ITEMS_JSON" | jq -r --arg repo "$REPO_SLUG" '
  .items[]? | select(.content.type=="Issue") | select((.content.repository // "") == $repo) | .content.number
' | tr '\n' ' ')

STATE_FILTER="--state open"
[[ $INCLUDE_ALL -eq 1 ]] && STATE_FILTER="--state all"

ISSUES_JSON=$(gh issue list $STATE_FILTER --limit 2000 --json number,title,url,state)

added=0
skipped=0
total=$(echo "$ISSUES_JSON" | jq 'length')

while IFS= read -r row; do
  num=$(echo "$row" | jq -r '.number')
  url=$(echo "$row" | jq -r '.url')
  state=$(echo "$row" | jq -r '.state')
  if echo " $EXISTING " | grep -q " $num "; then
    skipped=$((skipped+1))
    continue
  fi
  ITEM_JSON=$(gh project item-add "$NUM" --owner "$OWNER" --url "$url" --format json)
  ITEM_ID=$(echo "$ITEM_JSON" | jq -r '.id')
  if [[ -n "$ITEM_ID" ]]; then
    gh project item-edit --project-id "$PROJECT_ID" --id "$ITEM_ID" --field-id "$STATUS_FIELD_ID" --single-select-option-id "$BACKLOG_OPTION_ID" >/dev/null 2>&1 || true
    added=$((added+1))
    echo "Added issue #$num → Backlog" >&2
  fi
done < <(echo "$ISSUES_JSON" | jq -rc '.[]')

echo "Done. total=$total added=$added skipped=$skipped" >&2
