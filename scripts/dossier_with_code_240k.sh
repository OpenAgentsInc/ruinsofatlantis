#!/usr/bin/env bash
# Generate a 240 KiB "Comprehensive Dossier — with code" bundle, save it under
# docs/dossiers/generated/<topic>.with-code.md, and copy it to the clipboard.
#
# Usage:
#   scripts/dossier_with_code_240k.sh <topic-slug>
#
# Presets (topic-slug):
#   wyvern-viewer-anims   → model viewer + shared/assets animation loading

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
LIMIT_BYTES=$((240*1024))
TOPIC="${1:-}"
if [ -z "$TOPIC" ]; then
  echo "usage: $0 <topic-slug>" >&2; exit 2
fi

OUT_DIR="$ROOT_DIR/docs/dossiers/generated"
mkdir -p "$OUT_DIR"
OUT_FILE="$OUT_DIR/${TOPIC}.with-code.md"

CLIP=""
if command -v pbcopy >/dev/null 2>&1; then CLIP=pbcopy; fi
if [ -z "$CLIP" ] && command -v xclip >/dev/null 2>&1; then CLIP=xclip; fi
if [ -z "$CLIP" ] && command -v wl-copy >/dev/null 2>&1; then CLIP=wl-copy; fi
if [ -z "$CLIP" ]; then echo "warning: no clipboard tool found (pbcopy/xclip/wl-copy)" >&2; fi

tmp_all="$(mktemp)"; tmp_trunc=""; trap 'rm -f "$tmp_all" ${tmp_trunc:+"$tmp_trunc"}' EXIT

add_file() {
  local p="$1"; local max="$2"; [ -f "$p" ] || return 0
  local abs_dir abs rel; abs_dir=$(cd "$(dirname "$p")" && pwd)
  abs="$abs_dir/$(basename "$p")"; rel="${abs#"$ROOT_DIR/"}"
  printf '\n\n===== %s =====\n\n' "$rel" >> "$tmp_all"
  sed -n "1,${max}p" "$p" >> "$tmp_all" || true
}

header() {
  cat >> "$tmp_all" <<EOF
# Comprehensive Dossier — with code (${TOPIC})

This bundle contains the narrative dossier plus the most relevant source files
and docs inlined below (truncated to fit ${LIMIT_BYTES} bytes total).

EOF
}

case "$TOPIC" in
  wyvern-viewer-anims)
    header
    # Narrative dossier reference
    add_file "$ROOT_DIR/docs/dossiers/red-wyvern-loading.md" 8000
    # Viewer sources
    add_file "$ROOT_DIR/tools/model-viewer/src/main.rs" 4000
    add_file "$ROOT_DIR/tools/model-viewer/src/shader_skinned.wgsl" 4000
    # Shared assets used by viewer animation path
    add_file "$ROOT_DIR/shared/assets/src/skinning.rs" 4000
    add_file "$ROOT_DIR/shared/assets/src/retarget.rs" 4000
    add_file "$ROOT_DIR/shared/assets/src/types.rs" 2000
    add_file "$ROOT_DIR/shared/assets/src/util.rs" 1500
    # Docs for context
    add_file "$ROOT_DIR/docs/graphics/model-viewer.md" 3000
    ;;
  *)
    echo "error: unknown topic preset: $TOPIC" >&2; exit 2 ;;
esac

# Enforce size cap
size_bytes=$(wc -c < "$tmp_all" | tr -d ' ')
if (( size_bytes > LIMIT_BYTES )); then
  tmp_trunc="$(mktemp)"
  head -c "$LIMIT_BYTES" "$tmp_all" > "$tmp_trunc"
  mv "$tmp_trunc" "$tmp_all"
  size_bytes=$LIMIT_BYTES
fi

cp "$tmp_all" "$OUT_FILE"
if [ -n "$CLIP" ]; then
  case "$CLIP" in
    pbcopy) pbcopy < "$OUT_FILE" ;;
    xclip)  xclip -selection clipboard < "$OUT_FILE" ;;
    wl-copy) wl-copy < "$OUT_FILE" ;;
  esac
fi

kb=$(awk -v b="$size_bytes" 'BEGIN{printf "%.1f", b/1024}')
echo "Wrote $OUT_FILE (${kb} KiB). Copied to clipboard if available."

