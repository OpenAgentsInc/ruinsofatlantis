#!/usr/bin/env bash
# Re-exec under bash if invoked with sh
if [ -z "${BASH_VERSION:-}" ]; then
  if command -v bash >/dev/null 2>&1; then
    exec bash "$0" "$@"
  fi
fi
set -euo pipefail

# Copy to clipboard all files relevant to in-world placement ("worldsmithing")
# including renderer placement paths for trees/rocks, the platform builder overlay,
# and the entire worldsmithing crate. Files are separated by a header line.
# If the concatenated blob exceeds 240 KiB, truncate to that size.

ROOT_DIR="/Users/christopherdavid/code/ruinsofatlantis"
LIMIT_BYTES=$((240*1024))

if [[ ! -d "$ROOT_DIR" ]]; then
  echo "error: repo root not found: $ROOT_DIR" >&2
  exit 1
fi

# Detect clipboard tool
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

declare -a CURATED=()
add() {
  local p="$1"
  if [[ -f "$p" ]]; then CURATED+=("$p"); fi
}

# Platform: builder overlay + event integration
add "$ROOT_DIR/crates/platform_winit/src/lib.rs"

# Renderer: placement and preview paths
add "$ROOT_DIR/crates/render_wgpu/src/gfx/mod.rs"
add "$ROOT_DIR/crates/render_wgpu/src/gfx/foliage.rs"
add "$ROOT_DIR/crates/render_wgpu/src/gfx/rocks.rs"
add "$ROOT_DIR/crates/render_wgpu/src/gfx/scene.rs"
add "$ROOT_DIR/crates/render_wgpu/src/gfx/renderer/render.rs"
add "$ROOT_DIR/crates/render_wgpu/src/gfx/renderer/update/builder.rs"

# Data runtime: worldsmithing policy in zone manifest
add "$ROOT_DIR/crates/data_runtime/src/zone.rs"
if [[ -f "$ROOT_DIR/data/zones/campaign_builder/manifest.json" ]]; then
  CURATED+=("$ROOT_DIR/data/zones/campaign_builder/manifest.json")
fi

# Worldsmithing crate: include everything
if [[ -d "$ROOT_DIR/crates/worldsmithing" ]]; then
  while IFS= read -r f; do CURATED+=("$f"); done < <(find "$ROOT_DIR/crates/worldsmithing" -type f \( -name "*.rs" -o -name "Cargo.toml" \))
fi

# Dynamic search to catch additional relevant files
declare -a SEARCH_DIRS=(
  "$ROOT_DIR/crates/render_wgpu/src"
  "$ROOT_DIR/crates/platform_winit/src"
  "$ROOT_DIR/crates/data_runtime/src"
  "$ROOT_DIR/crates/worldsmithing"
)
declare -a PATTERNS=(
  '\\bworldsmithing\\b'
  '\\bplace(ment)?\\b'
  '\\bghost\\b'
  '\\bfoliage\\b'
  '\\brocks?\\b'
  '\\btrees?\\b'
  '\\bbuilder\\b'
)

declare -a FOUND=()
for dir in "${SEARCH_DIRS[@]}"; do
  [[ -d "$dir" ]] || continue
  for pat in "${PATTERNS[@]}"; do
    while IFS= read -r f; do FOUND+=("$f"); done < <(rg -n -l -i -g '!**/target/**' -g '!**/.git/**' "$pat" "$dir" || true)
  done
done

# Merge and uniq
{
  for f in "${CURATED[@]}"; do printf '%s\n' "$f"; done
  # Guard in case FOUND is empty/unset in some shells
  if [[ ${#FOUND[@]:-0} -gt 0 ]]; then
    for f in "${FOUND[@]}"; do printf '%s\n' "$f"; done
  fi
} | awk 'NF' | LC_ALL=C sort -u > /tmp/worldsmithing_files_$$.list

FILES_LIST=/tmp/worldsmithing_files_$$.list
if [[ ! -s "$FILES_LIST" ]]; then
  echo "error: no files found to copy" >&2
  exit 1
fi

tmp_all="$(mktemp)"
trap 'rm -f "$tmp_all" "$FILES_LIST"; [[ -n "${tmp_trunc:-}" ]] && rm -f "$tmp_trunc"' EXIT

while IFS= read -r file; do
  [[ -f "$file" ]] || continue
  abs_dir="$(cd "$(dirname "$file")" && pwd)"
  abs="$abs_dir/$(basename "$file")"
  printf '%s\n' "----- $abs -----" >> "$tmp_all"
  cat "$file" >> "$tmp_all" || true
  printf '\n' >> "$tmp_all"
done < "$FILES_LIST"

size_bytes=$(wc -c < "$tmp_all" | tr -d ' ')
truncated="no"
if (( size_bytes > LIMIT_BYTES )); then
  tmp_trunc="$(mktemp)"
  head -c "$LIMIT_BYTES" "$tmp_all" > "$tmp_trunc"
  size_bytes=$LIMIT_BYTES
  mv "$tmp_trunc" "$tmp_all"
  truncated="yes"
fi

case "$CLIP_TOOL" in
  pbcopy) pbcopy < "$tmp_all" ;;
  xclip)  xclip -selection clipboard < "$tmp_all" ;;
  wl-copy) wl-copy < "$tmp_all" ;;
esac

size_kb=$(awk -v b="$size_bytes" 'BEGIN { printf "%.1f", b/1024 }')
files_count=$(wc -l < "$FILES_LIST" | tr -d ' ')
echo "Copied ${size_kb} KiB to clipboard from ${files_count} files (truncated=${truncated})"
