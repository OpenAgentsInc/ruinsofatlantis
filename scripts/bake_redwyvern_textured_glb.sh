#!/usr/bin/env bash
# Bake Red Wyvern UDIM textures into single 0–1 albedo maps per material
# and export a textured GLB the viewer can display. Requires Blender.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BLENDER=${BLENDER:-blender}
SIZE=${SIZE:-4096}
OUT_GLb="${OUT_GLb:-$ROOT_DIR/assets/models/red_wyvern/RedDragon2021.textured.glb}"

# Try common Desktop paths for the source .blend
guess_blend() {
  for f in \
    "$HOME/Desktop/RedWyvern/uploads_files_2877852_FireBreathingWyvernDragon(update).blend" \
    "$HOME/Desktop/RedWyvern/uploads_files_2877852_FireBreathingWyvernDragon.blend"; do
    [ -f "$f" ] && { echo "$f"; return; }
  done
}

SRC_BLEND=${1:-}
if [ -z "$SRC_BLEND" ]; then SRC_BLEND=$(guess_blend || true); fi
if [ -z "$SRC_BLEND" ] || [ ! -f "$SRC_BLEND" ]; then
  echo "error: source .blend not found. Pass path as first arg." >&2
  exit 1
fi

if ! command -v "$BLENDER" >/dev/null 2>&1; then
  echo "error: Blender not found. Install and expose via \$BLENDER or PATH." >&2
  exit 1
fi

PY="$ROOT_DIR/scripts/blender/bake_redwyvern_textures.py"
if [ ! -f "$PY" ]; then
  echo "error: helper python not found: $PY" >&2
  exit 1
fi

mkdir -p "$(dirname "$OUT_GLb")"
"$BLENDER" -b "$SRC_BLEND" --python "$PY" -- \
  --size "$SIZE" --out "$OUT_GLb"
echo "Wrote $OUT_GLb"

