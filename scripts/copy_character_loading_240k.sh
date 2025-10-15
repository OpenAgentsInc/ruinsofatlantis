#!/usr/bin/env bash
set -eo pipefail

# Minimal character-loading bundle (<= 240 KiB)
# - Pulls only the most relevant snippets for swapping the PC model
# - Focus: where the skinned model is loaded/bound and core types

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
LIMIT=$((240*1024))

# Clipboard tool
CLIP=""; command -v pbcopy >/dev/null 2>&1 && CLIP=pbcopy
[[ -z "$CLIP" && -x "$(command -v xclip 2>/dev/null || true)" ]] && CLIP=xclip
[[ -z "$CLIP" && -x "$(command -v wl-copy 2>/dev/null || true)" ]] && CLIP=wl-copy
if [[ -z "$CLIP" ]]; then echo "error: need pbcopy|xclip|wl-copy" >&2; exit 1; fi

tmp_all="$(mktemp)"; tmp_tmp=""; trap 'rm -f "$tmp_all" ${tmp_tmp:+"$tmp_tmp"} 2>/dev/null || true' EXIT

add_header() {
  printf '%s\n' "----- $1 -----" >> "$tmp_all"
}

add_file_small() {
  local p="$1"; [[ -f "$p" ]] || return 0
  local sz; sz=$(wc -c < "$p" | tr -d ' ')
  # Only include whole file if reasonably small (<= 30 KiB)
  if (( sz <= 30720 )); then
    add_header "$p"
    cat "$p" >> "$tmp_all"; printf '\n' >> "$tmp_all"
  fi
}

add_head() {
  local p="$1"; local n=${2:-400}; [[ -f "$p" ]] || return 0
  add_header "$p (first ${n} lines)"
  sed -n "1,${n}p" "$p" >> "$tmp_all"; printf '\n' >> "$tmp_all"
}

# Context header
cat >> "$tmp_all" << 'CTX'
----- CONTEXT -----
Goal: Curated, minimal files to understand and swap the player character (skinned) model.
Focus on where the PC model is loaded/bound (SkinnedMeshCPU, load_gltf_skinned) and the
renderer paths that use it. This bundle is trimmed to <= 240 KiB for review.
----- END CONTEXT -----

CTX

# 1) Shared loader (core): skinning + facade (full if small)
add_file_small "$ROOT_DIR/shared/assets/src/skinning.rs"
add_file_small "$ROOT_DIR/shared/assets/src/lib.rs"

# 2) Renderer snippets where PC/NPC skinned assets are wired
add_head "$ROOT_DIR/crates/render_wgpu/src/gfx/renderer/init.rs" 420
add_head "$ROOT_DIR/crates/render_wgpu/src/gfx/renderer/render.rs" 320
add_head "$ROOT_DIR/crates/render_wgpu/src/gfx/pipeline.rs" 260
add_head "$ROOT_DIR/crates/render_wgpu/src/gfx/types.rs" 200

# 3) Example skinned actors (NPCs) — show pattern for swapping
add_head "$ROOT_DIR/crates/render_wgpu/src/gfx/deathknight.rs" 160
add_head "$ROOT_DIR/crates/render_wgpu/src/gfx/zombies.rs" 160
add_head "$ROOT_DIR/crates/render_wgpu/src/gfx/sorceress.rs" 160

# 4) Model viewer (first ~400 lines): helpful to test new GLB quickly
add_head "$ROOT_DIR/tools/model-viewer/src/main.rs" 300

# 5) Assets index (paths only, no binary contents)
add_header "assets/models/** (paths only)"
find "$ROOT_DIR/assets/models" -maxdepth 3 -type f \( -name "*.gltf" -o -name "*.glb" \) 2>/dev/null \
  | sed "s#^$ROOT_DIR/##" | LC_ALL=C sort >> "$tmp_all" || true
printf '\n' >> "$tmp_all"

# Truncate to limit
size=$(wc -c < "$tmp_all" | tr -d ' ')
if (( size > LIMIT )); then
  tmp_tmp="$(mktemp)"
  head -c "$LIMIT" "$tmp_all" > "$tmp_tmp"
  mv "$tmp_tmp" "$tmp_all"
  size=$LIMIT
  truncated=yes
else
  truncated=no
fi

# Copy to clipboard
case "$CLIP" in
  pbcopy) pbcopy < "$tmp_all" ;;
  xclip)  xclip -selection clipboard < "$tmp_all" ;;
  wl-copy) wl-copy < "$tmp_all" ;;
esac

kb=$(awk -v b="$size" 'BEGIN{printf "%.1f", b/1024}')
echo "Copied ${kb} KiB to clipboard (truncated=${truncated})"
