#!/usr/bin/env bash
# Copy up to 240 KiB of Red Wyvern loading context to the clipboard.
# Includes the dossier, referenced code files, and an asset-path index.

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
  # Cap per-file to a large line budget (sane for source/doc files)
  sed -n '1,4000p' "$p" >> "$tmp_all" || true
  printf '\n' >> "$tmp_all"
}

# Context header
cat >> "$tmp_all" << 'CTX'
----- CONTEXT -----
Bundle: Red Wyvern Loading — code, scripts, and docs referenced by the
comprehensive dossier, limited to 240 KiB for quick sharing.

Contents
- Dossier: docs/dossiers/red-wyvern-loading.md
- Viewer: tools/model-viewer (main.rs, skinned WGSL)
- Loaders: shared/assets (skinning, draco, util, types, retarget)
- Docs: model-loader overview and viewer docs
- Blender: headless export script to produce a packed GLB
- Assets index (paths only; no binary contents)
----- END CONTEXT -----

CTX

# Dossier
add_file "$ROOT_DIR/docs/dossiers/red-wyvern-loading.md"

# Viewer sources
add_file "$ROOT_DIR/tools/model-viewer/src/main.rs"
add_file "$ROOT_DIR/tools/model-viewer/src/shader_skinned.wgsl"

# Shared assets (loaders + helpers)
add_file "$ROOT_DIR/shared/assets/src/skinning.rs"
add_file "$ROOT_DIR/shared/assets/src/draco.rs"
add_file "$ROOT_DIR/shared/assets/src/util.rs"
add_file "$ROOT_DIR/shared/assets/src/types.rs"
add_file "$ROOT_DIR/shared/assets/src/retarget.rs"
add_file "$ROOT_DIR/shared/assets/src/gltf.rs"
add_file "$ROOT_DIR/shared/assets/src/lib.rs"

# Docs
add_file "$ROOT_DIR/docs/graphics/model-viewer.md"
add_file "$ROOT_DIR/docs/gdd/11-technical/graphics/model-loading.md"

# Blender export helper
add_file "$ROOT_DIR/scripts/blender/export_glb_clean.py"

# Engine integration (renderer)
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/wyvern.rs"
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/renderer/init.rs"
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/renderer/passes.rs"
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/draw.rs"
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/mod.rs"
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/shader.wgsl"
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/pipeline.rs"

# Zone manifest used to enable wyvern in cc_demo
add_file "$ROOT_DIR/data/zones/cc_demo/manifest.json"

# Asset index (paths only; do NOT include binary contents)
{
  printf '%s\n' "----- assets index (paths only) -----"
  # Red Wyvern model + UDIMs (paths only)
  find "$ROOT_DIR/assets/models/red_wyvern" -maxdepth 3 -type f 2>/dev/null | \
    sed "s#^$ROOT_DIR/##" | LC_ALL=C sort || true
  # Animation libraries for dragons + converted
  find "$ROOT_DIR/assets/anims/dragons" -maxdepth 3 -type f \( -name '*.fbx' -o -name '*.gltf' -o -name '*.glb' \) 2>/dev/null | \
    sed "s#^$ROOT_DIR/##" | LC_ALL=C sort || true
  find "$ROOT_DIR/assets/anims/converted" -maxdepth 1 -type f \( -name '*RedDragon*.glb' -o -name '*RedWyvern*.glb' \) 2>/dev/null | \
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
