#!/usr/bin/env bash
# Copy to clipboard up to 240 KiB of context for the model viewer:
# - tools/model-viewer sources (main.rs + WGSL)
# - shared/assets skinning + FBX/GLTF merge utilities + types facade
# - model viewer docs
# - any other files that reference core animation merge/loading helpers
# - index (paths only) of relevant assets under assets/models and assets/anims

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
LIMIT_BYTES=$((240*1024))

# Clipboard tool
CLIP=""
if command -v pbcopy >/dev/null 2>&1; then CLIP=pbcopy; fi
if [ -z "$CLIP" ] && command -v xclip >/dev/null 2>&1; then CLIP=xclip; fi
if [ -z "$CLIP" ] && command -v wl-copy >/dev/null 2>&1; then CLIP=wl-copy; fi
if [ -z "$CLIP" ]; then echo "error: need pbcopy|xclip|wl-copy" >&2; exit 1; fi

tmp_all="$(mktemp)"; tmp_trunc=""
trap 'rm -f "$tmp_all" ${tmp_trunc:+"$tmp_trunc"} 2>/dev/null || true' EXIT

add_file() {
  local p="$1"
  [ -f "$p" ] || return 0
  local abs_dir abs
  abs_dir=$(cd "$(dirname "$p")" && pwd)
  abs="$abs_dir/$(basename "$p")"
  printf '%s\n' "----- $abs -----" >> "$tmp_all"
  # Cap per-file lines generously to avoid blowing the limit with huge files
  sed -n '1,4000p' "$p" >> "$tmp_all" || true
  printf '\n' >> "$tmp_all"
}

# Context header
cat >> "$tmp_all" << 'CTX'
----- CONTEXT -----
Goal: Collect all code paths related to the standalone model viewer and
animation merging (GLTF/FBX), including CPU loaders and a quick assets index,
so we can debug and iterate on imports quickly. Contents:
- tools/model-viewer sources (main.rs + WGSL)
- shared/assets loaders and animation merge (GLTF + FBX stubs)
- docs/graphics/model-viewer.md (architecture & debug guide)
- additional files that reference merge_gltf_animations / load_gltf_skinned
- assets index (paths only) under assets/models and assets/anims
----- END CONTEXT -----

CTX

# Curated files — model viewer
add_file "$ROOT_DIR/tools/model-viewer/src/main.rs"
add_file "$ROOT_DIR/tools/model-viewer/src/shader_skinned.wgsl"
add_file "$ROOT_DIR/tools/model-viewer/src/shader_basic.wgsl"

# Shared assets (loaders + types)
add_file "$ROOT_DIR/shared/assets/src/skinning.rs"
add_file "$ROOT_DIR/shared/assets/src/fbx.rs"
add_file "$ROOT_DIR/shared/assets/src/types.rs"
add_file "$ROOT_DIR/shared/assets/src/lib.rs"

# Docs
add_file "$ROOT_DIR/docs/graphics/model-viewer.md"

# Grep-based discovery for helpers that the viewer relies on
if command -v rg >/dev/null 2>&1; then
  rg -n -l -S \
    -e 'merge_gltf_animations' \
    -e 'merge_fbx_animations' \
    -e 'try_convert_fbx_to_gltf' \
    -e 'load_gltf_skinned' \
    "$ROOT_DIR/crates" "$ROOT_DIR/shared" "$ROOT_DIR/tools" 2>/dev/null | \
    LC_ALL=C sort -u | while IFS= read -r f; do add_file "$f"; done
else
  grep -R -n -E 'merge_gltf_animations|merge_fbx_animations|try_convert_fbx_to_gltf|load_gltf_skinned' \
    "$ROOT_DIR/crates" "$ROOT_DIR/shared" "$ROOT_DIR/tools" 2>/dev/null | cut -d: -f1 | \
    LC_ALL=C sort -u | while IFS= read -r f; do add_file "$f"; done
fi

# Asset index (paths only; do NOT include binary contents)
{
  printf '%s\n' "----- assets index (paths only) -----"
  find "$ROOT_DIR/assets/models" -maxdepth 3 -type f \( -name '*.gltf' -o -name '*.glb' \) 2>/dev/null | \
    sed "s#^$ROOT_DIR/##" | LC_ALL=C sort || true
  find "$ROOT_DIR/assets/anims" -maxdepth 3 -type f \( -name '*.gltf' -o -name '*.glb' -o -name '*.fbx' \) 2>/dev/null | \
    sed "s#^$ROOT_DIR/##" | LC_ALL=C sort || true
  printf '\n'
} >> "$tmp_all"

# Truncate to limit
size_bytes=$(wc -c < "$tmp_all" | tr -d ' ')
truncated=no
if (( size_bytes > LIMIT_BYTES )); then
  tmp_trunc="$(mktemp)"
  head -c "$LIMIT_BYTES" "$tmp_all" > "$tmp_trunc"
  mv "$tmp_trunc" "$tmp_all"
  size_bytes=$LIMIT_BYTES
  truncated=yes
fi

# Clipboard copy
case "$CLIP" in
  pbcopy) pbcopy < "$tmp_all" ;;
  xclip)  xclip -selection clipboard < "$tmp_all" ;;
  wl-copy) wl-copy < "$tmp_all" ;;
esac

kb=$(awk -v b="$size_bytes" 'BEGIN{printf "%.1f", b/1024}')
echo "Copied ${kb} KiB to clipboard (truncated=${truncated})"

