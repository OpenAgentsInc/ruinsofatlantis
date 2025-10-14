#!/usr/bin/env bash
# Re-exec under bash if invoked with sh
if [ -z "${BASH_VERSION:-}" ]; then
  if command -v bash >/dev/null 2>&1; then
    exec bash "$0" "$@"
  fi
fi
set -euo pipefail

# Copies the concatenated contents of all files under the renderer folder
# to the system clipboard, separating files with a header line. If the
# concatenated blob exceeds 240 KiB, it is truncated to that size.

BASE_DIR="/Users/christopherdavid/code/ruinsofatlantis/crates/render_wgpu/src/gfx/renderer"
LIMIT_BYTES=$((240*1024))

if [[ ! -d "$BASE_DIR" ]]; then
  echo "error: renderer directory not found: $BASE_DIR" >&2
  exit 1
fi

# Detect clipboard tool (just the program name; flags handled in case)
CLIP_TOOL=""
if command -v pbcopy >/dev/null 2>&1; then
  CLIP_TOOL="pbcopy"
elif command -v xclip >/dev/null 2>&1; then
  CLIP_TOOL="xclip"
elif command -v wl-copy >/dev/null 2>&1; then
  CLIP_TOOL="wl-copy"
else
  echo "error: no clipboard tool found (need pbcopy, xclip, or wl-copy)" >&2
  exit 1
fi

tmp_all="$(mktemp)"
# shellcheck disable=SC2064
trap 'rm -f "$tmp_all"; [[ -n "${tmp_trunc:-}" ]] && rm -f "$tmp_trunc"' EXIT

# Concatenate files with headers. Use predictable sorted order.
# macOS/BSD sort doesn't support -z reliably; use newline-delimited list.
find "$BASE_DIR" -type f | LC_ALL=C sort | while IFS= read -r file; do
  abs_dir="$(cd "$(dirname "$file")" && pwd)"
  abs="$abs_dir/$(basename "$file")"
  printf '%s\n' "----- $abs -----" >> "$tmp_all"
  cat "$file" >> "$tmp_all" || true
  printf '\n' >> "$tmp_all"
done

size_bytes=$(wc -c < "$tmp_all" | tr -d ' ')
truncated="no"
if (( size_bytes > LIMIT_BYTES )); then
  tmp_trunc="$(mktemp)"
  head -c "$LIMIT_BYTES" "$tmp_all" > "$tmp_trunc"
  size_bytes=$LIMIT_BYTES
  mv "$tmp_trunc" "$tmp_all"
  truncated="yes"
fi

# Copy to clipboard
case "$CLIP_TOOL" in
  pbcopy)
    pbcopy < "$tmp_all" ;;
  xclip)
    xclip -selection clipboard < "$tmp_all" ;;
  wl-copy)
    wl-copy < "$tmp_all" ;;
esac

size_kb=$(awk -v b="$size_bytes" 'BEGIN { printf "%.1f", b/1024 }')
echo "Copied ${size_kb} KiB to clipboard (truncated=${truncated})"
