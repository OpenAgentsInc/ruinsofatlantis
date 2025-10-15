#!/usr/bin/env sh
set -eu

# Copy to clipboard all files relevant to character loading (PC/NPC skinned models),
# pipelines, and loaders, so we can swap models (e.g., warrior/barbarian) easily.
# Includes renderer init/draw, shared/assets skinning loader, types, pipelines, and model viewer.
# Truncates to 2,140 KiB.

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
LIMIT=$((2140*1024))

# Detect clipboard tool
CLIP=""
if command -v pbcopy >/dev/null 2>&1; then CLIP=pbcopy; fi
if [ -z "$CLIP" ] && command -v xclip >/dev/null 2>&1; then CLIP=xclip; fi
if [ -z "$CLIP" ] && command -v wl-copy >/dev/null 2>&1; then CLIP=wl-copy; fi
if [ -z "$CLIP" ]; then echo "error: need pbcopy|xclip|wl-copy" >&2; exit 1; fi

tmp_all="$(mktemp)"; tmp_trunc=""; trap 'rm -f "$tmp_all" ${tmp_trunc:+"$tmp_trunc"} 2>/dev/null || true' EXIT

add_file() {
  p="$1"; [ -f "$p" ] || return 0
  abs_dir=$(cd "$(dirname "$p")" && pwd)
  abs="$abs_dir/$(basename "$p")"
  printf '%s\n' "----- $abs -----" >> "$tmp_all"
  sed -n '1,2000p' "$p" >> "$tmp_all" || true
  printf '\n' >> "$tmp_all"
}

# Context header
cat >> "$tmp_all" << 'CTX'
----- CONTEXT -----
Goal: Collect all code paths that load and render the player character (skinned models) so we can swap the PC model (e.g., warrior/barbarian) instead of the current default. This bundle includes:
- Renderer init and draw for skinned actors (PC/NPC), pipelines, types
- Shared/assets skinning loader (GLTF → SkinnedMeshCPU)
- Model viewer (for offline inspection of GLB/GLTF)
- Modules that reference wizard/zombie/sorceress/deathknight assets as examples
- A quick index of common assets paths under assets/models/**
----- END CONTEXT -----

CTX

# Curated renderer files
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/renderer/init.rs"
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/renderer/render.rs"
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/renderer/passes.rs"
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/pipeline.rs"
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/types.rs"
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/scene.rs"
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/deathknight.rs"
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/zombies.rs"
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/sorceress.rs"

# Shared assets: skinning + lib facade
add_file "$ROOT_DIR/shared/assets/src/skinning.rs"
add_file "$ROOT_DIR/shared/assets/src/lib.rs"

# Model viewer (handy to validate replacements)
add_file "$ROOT_DIR/tools/model-viewer/src/main.rs"

# Data/runtime spots that may influence class/appearance
add_file "$ROOT_DIR/crates/data_runtime/src/loader.rs"
add_file "$ROOT_DIR/crates/data_runtime/src/class.rs"

# Quick grep-based discovery: load_gltf_skinned, SkinnedMeshCPU, wizard_vb/pc_vb
if command -v rg >/dev/null 2>&1; then
  rg -n -l -S \
    -e 'load_gltf_skinned' \
    -e 'SkinnedMeshCPU' \
    -e '\bpc_vb\b|\bpc_cpu\b|\bpc_mat_bg\b' \
    -e '\bwizard_vb\b|\bzombie_vb\b|\bsorc_vb\b|\bdk_vb\b' \
    "$ROOT_DIR/crates" "$ROOT_DIR/shared" 2>/dev/null | sort -u | while IFS= read -r f; do add_file "$f"; done
else
  grep -R -n -E 'load_gltf_skinned|SkinnedMeshCPU|\bpc_vb\b|\bpc_cpu\b|\bpc_mat_bg\b|\bwizard_vb\b|\bzombie_vb\b|\bsorc_vb\b|\bdk_vb\b' \
    "$ROOT_DIR/crates" "$ROOT_DIR/shared" 2>/dev/null | cut -d: -f1 | sort -u | while IFS= read -r f; do add_file "$f"; done
fi

# Asset index (paths only, not file contents)
printf '%s\n' "----- assets index (paths only) -----" >> "$tmp_all"
find "$ROOT_DIR/assets/models" -maxdepth 3 -type f \( -name "*.gltf" -o -name "*.glb" \) 2>/dev/null \
  | sed "s#^$ROOT_DIR/##" | LC_ALL=C sort >> "$tmp_all" || true
printf '\n' >> "$tmp_all"

# Truncate to limit
size=$(wc -c < "$tmp_all" | tr -d ' ')
if [ "$size" -gt "$LIMIT" ]; then
  tmp_trunc="$(mktemp)"
  head -c "$LIMIT" "$tmp_all" > "$tmp_trunc"
  mv "$tmp_trunc" "$tmp_all"
  size=$LIMIT
  truncated=yes
else
  truncated=no
fi

# Copy
case "$CLIP" in
  pbcopy) pbcopy < "$tmp_all" ;;
  xclip) xclip -selection clipboard < "$tmp_all" ;;
  wl-copy) wl-copy < "$tmp_all" ;;
esac

kb=$(awk -v b="$size" 'BEGIN{printf "%.1f", b/1024}')
echo "Copied ${kb} KiB to clipboard (truncated=${truncated})"
