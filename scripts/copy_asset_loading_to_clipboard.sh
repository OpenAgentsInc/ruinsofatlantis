#!/usr/bin/env bash
# Re-exec under bash if invoked with sh
if [ -z "${BASH_VERSION:-}" ]; then
  if command -v bash >/dev/null 2>&1; then
    exec bash "$0" "$@"
  fi
fi
set -euo pipefail

# Copy to clipboard the concatenated contents of all files relevant to
# asset/zone loading, background loading, and the HUD loading overlay.
# Files are separated by a header line: "----- {abs path} -----".
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

# Seed curated paths we know are involved in zone switching, scene assembly,
# and asset loading.
declare -a CURATED=()
add() {
  local p="$1"
  if [[ -f "$p" ]]; then
    CURATED+=("$p")
  fi
}

# Platform: zone picker + event loop
add "$ROOT_DIR/crates/platform_winit/src/lib.rs"

# Renderer core & scene assembly
add "$ROOT_DIR/crates/render_wgpu/src/gfx/mod.rs"
add "$ROOT_DIR/crates/render_wgpu/src/gfx/init.rs"
add "$ROOT_DIR/crates/render_wgpu/src/gfx/render.rs"
add "$ROOT_DIR/crates/render_wgpu/src/gfx/scene.rs"
add "$ROOT_DIR/crates/render_wgpu/src/gfx/zone_batches.rs"
add "$ROOT_DIR/crates/render_wgpu/src/gfx/foliage.rs"
add "$ROOT_DIR/crates/render_wgpu/src/gfx/rocks.rs"
add "$ROOT_DIR/crates/render_wgpu/src/gfx/ruins.rs"
add "$ROOT_DIR/crates/render_wgpu/src/gfx/material.rs"
add "$ROOT_DIR/crates/render_wgpu/src/gfx/voxel_upload.rs"

# HUD logic that could display loading states
add "$ROOT_DIR/crates/ux_hud/src/lib.rs"
add "$ROOT_DIR/crates/ux_hud/src/hud.rs"

# Asset loaders (shared/assets aka roa-assets)
if [[ -d "$ROOT_DIR/shared/assets/src" ]]; then
  while IFS= read -r f; do CURATED+=("$f"); done < <(find "$ROOT_DIR/shared/assets/src" -type f -name "*.rs")
fi

# Data runtime (zone manifests, configs)
if [[ -d "$ROOT_DIR/crates/data_runtime/src" ]]; then
  while IFS= read -r f; do CURATED+=("$f"); done < <(find "$ROOT_DIR/crates/data_runtime/src" -type f -name "*.rs")
  # add schemas and example manifests if present
  if [[ -d "$ROOT_DIR/crates/data_runtime/schemas" ]]; then
    while IFS= read -r f; do CURATED+=("$f"); done < <(find "$ROOT_DIR/crates/data_runtime/schemas" -type f \( -name "*.json" -o -name "*.ron" -o -name "*.toml" \))
  fi
fi

# Dynamic search for additional relevant files by keywords across key crates.
declare -a SEARCH_DIRS=(
  "$ROOT_DIR/crates/render_wgpu/src"
  "$ROOT_DIR/crates/platform_winit/src"
  "$ROOT_DIR/crates/ux_hud/src"
  "$ROOT_DIR/shared/assets/src"
  "$ROOT_DIR/crates/data_runtime/src"
)

declare -a PATTERNS=(
  'roa[_-]assets'
  '\\bgltf\\b'
  '\\bload(ing)?\\b'
  '\\bLoader\\b'
  '\\bpacks?\\b'
  '\\bzones?\\b'
  '\\bmanifest\\b'
  '\\battach_zone\\b'
  '\\bZonePicker\\b'
  '\\bHUD\\b'
  '\\bLoading\\b'
)

declare -a FOUND=()
for dir in "${SEARCH_DIRS[@]}"; do
  if [[ -d "$dir" ]]; then
    for pat in "${PATTERNS[@]}"; do
      while IFS= read -r f; do FOUND+=("$f"); done < <(rg -n -l -i -g '!**/target/**' -g '!**/.git/**' "$pat" "$dir" || true)
    done
  fi
done

# Merge, normalize, and de-duplicate
declare -a FILES=()
{
  for f in "${CURATED[@]}"; do printf '%s\n' "$f"; done
  for f in "${FOUND[@]}"; do printf '%s\n' "$f"; done
} | awk 'NF' | LC_ALL=C sort -u > /tmp/asset_files_$$.list

FILES_LIST=/tmp/asset_files_$$.list

if [[ ! -s "$FILES_LIST" ]]; then
  echo "error: no files found to copy" >&2
  exit 1
fi

tmp_all="$(mktemp)"
# shellcheck disable=SC2064
trap 'rm -f "$tmp_all"; [[ -n "${tmp_trunc:-}" ]] && rm -f "$tmp_trunc"' EXIT

while IFS= read -r file; do
  if [[ -f "$file" ]]; then
    abs_dir="$(cd "$(dirname "$file")" && pwd)"
    abs="$abs_dir/$(basename "$file")"
    printf '%s\n' "----- $abs -----" >> "$tmp_all"
    cat "$file" >> "$tmp_all" || true
    printf '\n' >> "$tmp_all"
  fi
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
  pbcopy)
    pbcopy < "$tmp_all" ;;
  xclip)
    xclip -selection clipboard < "$tmp_all" ;;
  wl-copy)
    wl-copy < "$tmp_all" ;;
esac

size_kb=$(awk -v b="$size_bytes" 'BEGIN { printf "%.1f", b/1024 }')
files_count=$(wc -l < "$FILES_LIST" | tr -d ' ')
rm -f "$FILES_LIST"
echo "Copied ${size_kb} KiB to clipboard from ${files_count} files (truncated=${truncated})"
