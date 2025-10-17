#!/usr/bin/env bash
# Re-exec under bash if invoked with sh
if [ -z "${BASH_VERSION:-}" ]; then
  if command -v bash >/dev/null 2>&1; then
    exec bash "$0" "$@"
  fi
fi
set -euo pipefail

# Copies the concatenated contents of all files related to the Octahedral
# impostor demo to the system clipboard, separating files with a header line.
# The copied blob is truncated to 240 KiB to keep it lightweight for chat tools.

LIMIT_BYTES=$((240*1024))

# Curated file list (update as needed when octa files move/change)
FILES=(
  \
  "/Users/christopherdavid/code/ruinsofatlantis/crates/render_wgpu/src/gfx/impostors.rs" \
  "/Users/christopherdavid/code/ruinsofatlantis/crates/render_wgpu/src/gfx/shader.wgsl" \
  "/Users/christopherdavid/code/ruinsofatlantis/crates/render_wgpu/src/gfx/renderer/passes.rs" \
  "/Users/christopherdavid/code/ruinsofatlantis/crates/render_wgpu/src/gfx/renderer/input.rs" \
  "/Users/christopherdavid/code/ruinsofatlantis/crates/render_wgpu/src/gfx/mod.rs" \
  "/Users/christopherdavid/code/ruinsofatlantis/crates/platform_winit/src/lib.rs" \
  "/Users/christopherdavid/code/ruinsofatlantis/scripts/fetch_horde_octa_assets.sh" \
)

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

tmp_all="$(mktemp)"
# shellcheck disable=SC2064
trap 'rm -f "$tmp_all"; [[ -n "${tmp_trunc:-}" ]] && rm -f "$tmp_trunc"' EXIT

# Concatenate files with absolute path headers; skip missing files gracefully
for f in "${FILES[@]}"; do
  if [[ -f "$f" ]]; then
    abs_dir="$(cd "$(dirname "$f")" && pwd)"
    abs="$abs_dir/$(basename "$f")"
    printf '%s\n' "----- $abs -----" >> "$tmp_all"
    cat "$f" >> "$tmp_all" || true
    printf '\n' >> "$tmp_all"
  fi
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

