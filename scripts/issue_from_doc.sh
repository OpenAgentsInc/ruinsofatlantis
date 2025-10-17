#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: $0 [--dry-run] <issue-markdown>

Parses simple front matter and creates a GitHub issue via gh CLI.
Front matter keys: title, labels (comma), assignees (comma), milestone

Optional env:
  PROJECT_OWNER   # org/user for GitHub Project (new Projects)
  PROJECT_NUMBER  # project number to add the created issue to
USAGE
}

DRY=0
if [[ "${1:-}" == "--dry-run" ]]; then
  DRY=1; shift || true
fi

FILE=${1:-}
[[ -n "$FILE" ]] || { usage; exit 1; }
[[ -f "$FILE" ]] || { echo "File not found: $FILE" >&2; exit 1; }

command -v gh >/dev/null 2>&1 || { echo "gh CLI is required: https://cli.github.com/" >&2; exit 1; }

# Find front matter bounds
START_LINE=$(awk '/^---[ \t]*$/{print NR; exit}' "$FILE" || true)
[[ -n "$START_LINE" ]] || { echo "Missing front matter start (---)" >&2; exit 1; }
END_LINE=$(awk -v s="$START_LINE" 'NR>s && /^---[ \t]*$/ {print NR; exit}' "$FILE" || true)
[[ -n "$END_LINE" ]] || { echo "Missing front matter end (---)" >&2; exit 1; }

FRONT=$(sed -n "$((START_LINE+1)),$((END_LINE-1))p" "$FILE")
BODY_FILE=$(mktemp)
trap 'rm -f "$BODY_FILE"' EXIT
sed -n "$((END_LINE+1)),999999p" "$FILE" > "$BODY_FILE"

get_val() {
  local key=$1
  printf '%s\n' "$FRONT" | awk -v k="$key" 'BEGIN{IGNORECASE=1} {
    if ($0 ~ "^" k ":[[:space:]]*") {
      line=$0; sub(/^[^:]*:[[:space:]]*/, "", line); print line; exit;
    }
  }'
}

TITLE=$(get_val title)
LABELS=$(get_val labels)
ASSIGNEES=$(get_val assignees)
MILESTONE=$(get_val milestone)

[[ -n "$TITLE" ]] || { echo "title is required in front matter" >&2; exit 1; }

echo "Parsed front matter:" >&2
echo "  title:      $TITLE" >&2
echo "  labels:     ${LABELS:-}" >&2
echo "  assignees:  ${ASSIGNEES:-}" >&2
echo "  milestone:  ${MILESTONE:-}" >&2

if [[ $DRY -eq 1 ]]; then
  echo "--dry-run; not creating issue" >&2
  exit 0
fi

args=(issue create --title "$TITLE" --body-file "$BODY_FILE")

if [[ -n "${ASSIGNEES:-}" ]]; then
  IFS=',' read -r -a ass_arr <<< "$ASSIGNEES"
  for a in "${ass_arr[@]}"; do
    a_trimmed=$(echo "$a" | xargs)
    [[ -n "$a_trimmed" ]] && args+=(--assignee "$a_trimmed")
  done
fi

if [[ -n "${MILESTONE:-}" ]]; then
  args+=(--milestone "$MILESTONE")
fi

# Create the issue; capture URL
ISSUE_URL=$(gh "${args[@]}" | tail -n1)
echo "$ISSUE_URL"
ISSUE_NUMBER=$(basename "$ISSUE_URL")

# Apply labels after creation; create labels if missing
if [[ -n "${LABELS:-}" ]]; then
  IFS=',' read -r -a labels_arr <<< "$LABELS"
  for lbl in "${labels_arr[@]}"; do
    lbl_trimmed=$(echo "$lbl" | xargs)
    [[ -z "$lbl_trimmed" ]] && continue
    if ! gh issue edit "$ISSUE_NUMBER" --add-label "$lbl_trimmed" >/dev/null 2>&1; then
      # Create label then try again
      gh label create "$lbl_trimmed" --color BFDADC --description "auto-created from docs/issues" >/dev/null 2>&1 || true
      gh issue edit "$ISSUE_NUMBER" --add-label "$lbl_trimmed" >/dev/null 2>&1 || true
    fi
  done
fi

# Optionally add to a GitHub Project (new Projects)
if [[ -n "${PROJECT_OWNER:-}" && -n "${PROJECT_NUMBER:-}" ]]; then
  ITEM_JSON=$(gh project item-add "$PROJECT_NUMBER" --owner "$PROJECT_OWNER" --url "$ISSUE_URL" --format json)
  ITEM_ID=$(echo "$ITEM_JSON" | jq -r '.id' 2>/dev/null || true)
  # Try to set Status=Backlog if available
  PROJECT_JSON=$(gh project view "$PROJECT_NUMBER" --owner "$PROJECT_OWNER" --format json)
  PROJECT_ID=$(echo "$PROJECT_JSON" | jq -r '.id' 2>/dev/null || true)
  FIELDS_JSON=$(gh project field-list "$PROJECT_NUMBER" --owner "$PROJECT_OWNER" --format json)
  STATUS_FIELD_ID=$(echo "$FIELDS_JSON" | jq -r '.fields[] | select(.name=="Status").id' 2>/dev/null || true)
  BACKLOG_OPTION_ID=$(echo "$FIELDS_JSON" | jq -r '.fields[] | select(.name=="Status").options[] | select(.name=="Backlog").id' 2>/dev/null || true)
  if [[ -n "$ITEM_ID" && -n "$PROJECT_ID" && -n "$STATUS_FIELD_ID" && -n "$BACKLOG_OPTION_ID" ]]; then
    gh project item-edit --project-id "$PROJECT_ID" --id "$ITEM_ID" --field-id "$STATUS_FIELD_ID" --single-select-option-id "$BACKLOG_OPTION_ID" >/dev/null 2>&1 || true
  fi
fi
