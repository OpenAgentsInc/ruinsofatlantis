#!/usr/bin/env sh
set -eu

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

# Temp files
LIST_CURATED="$(mktemp)"
LIST_FOUND="$(mktemp)"
trap 'rm -f "$LIST_CURATED" "$LIST_FOUND" "$tmp_all" "$tmp_trunc" 2>/dev/null || true' EXIT

add() {
  p="$1"
  if [ -f "$p" ]; then
    printf '%s\n' "$p" >> "$LIST_CURATED"
  fi
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
if [ -f "$ROOT_DIR/data/zones/campaign_builder/manifest.json" ]; then
  printf '%s\n' "$ROOT_DIR/data/zones/campaign_builder/manifest.json" >> "$LIST_CURATED"
fi

# Worldsmithing crate: include everything
if [ -d "$ROOT_DIR/crates/worldsmithing" ]; then
  find "$ROOT_DIR/crates/worldsmithing" -type f \( -name "*.rs" -o -name "Cargo.toml" \) -print >> "$LIST_CURATED"
fi

# Dynamic search to catch additional relevant files
SEARCH_DIRS="$ROOT_DIR/crates/render_wgpu/src
$ROOT_DIR/crates/platform_winit/src
$ROOT_DIR/crates/data_runtime/src
$ROOT_DIR/crates/worldsmithing"

# Prefer ripgrep; fallback to grep
if command -v rg >/dev/null 2>&1; then
  for dir in $SEARCH_DIRS; do
    [ -d "$dir" ] || continue
    rg -n -l -i -g '!**/target/**' -g '!**/.git/**' \
      -e '\\bworldsmithing\\b|\\bplace(ment)?\\b|\\bghost\\b|\\bfoliage\\b|\\brocks?\\b|\\btrees?\\b|\\bbuilder\\b' \
      "$dir" || true >> "$LIST_FOUND"
  done
else
  for dir in $SEARCH_DIRS; do
    [ -d "$dir" ] || continue
    grep -R -n -i -E '\\bworldsmithing\\b|\\bplace(ment)?\\b|\\bghost\\b|\\bfoliage\\b|\\brocks?\\b|\\btrees?\\b|\\bbuilder\\b' "$dir" 2>/dev/null | cut -d: -f1 >> "$LIST_FOUND" || true
  done
fi

# Merge and uniq
cat "$LIST_CURATED" "$LIST_FOUND" 2>/dev/null | awk 'NF' | LC_ALL=C sort -u > /tmp/worldsmithing_files_$$.list

FILES_LIST=/tmp/worldsmithing_files_$$.list
if [[ ! -s "$FILES_LIST" ]]; then
  echo "error: no files found to copy" >&2
  exit 1
fi

tmp_all="$(mktemp)"

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
