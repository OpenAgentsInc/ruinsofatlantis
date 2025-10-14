#!/usr/bin/env sh
set -eu

# Copy to clipboard up to 240 KiB of context: all files relevant to
# worldsmithing (builder, placement), the HUD/hotbar, and hotkey wiring.
# Adds a brief context header about the goal and observed behavior.

ROOT_DIR="/Users/christopherdavid/code/ruinsofatlantis"
LIMIT=$((240*1024))

# Detect clipboard tool
CLIP=""
if command -v pbcopy >/dev/null 2>&1; then CLIP=pbcopy; fi
if [ -z "$CLIP" ] && command -v xclip >/dev/null 2>&1; then CLIP=xclip; fi
if [ -z "$CLIP" ] && command -v wl-copy >/dev/null 2>&1; then CLIP=wl-copy; fi
if [ -z "$CLIP" ]; then echo "error: need pbcopy|xclip|wl-copy" >&2; exit 1; fi

tmp_all="$(mktemp)"; trap 'rm -f "$tmp_all" "$tmp_trunc" 2>/dev/null || true' EXIT

add_file() {
  p="$1"
  [ -f "$p" ] || return 0
  abs_dir=$(cd "$(dirname "$p")" && pwd)
  abs="$abs_dir/$(basename "$p")"
  printf '%s\n' "----- $abs -----" >> "$tmp_all"
  cat "$p" >> "$tmp_all" || true
  printf '\n' >> "$tmp_all"
}

# 1) Context header (what/why)
cat >> "$tmp_all" << 'CTX'
----- CONTEXT -----
Goal: Hotbar should show worldsmithing kinds (e.g., tree.birch, tree.giantpine, rock.building) when in Campaign Builder; builder keys (B/C toggle; 1/2/3 select; Enter place; ,/. rotate) should work on macOS with IME. Observed: hotbar currently shows spell slots (fire bolt, magic missile, fireball); pressing keys appears not to update hotbar nor place worldsmithing items.

We’re gathering all relevant files for the super agent to review: platform builder input (IME-safe), worldsmithing placement (session trees/rocks), HUD/hotbar build/draw paths, and the zone manifest policy (show_player_hud + kinds).
----- END CONTEXT -----

CTX

# 2) Curated files (explicit)
add_file "$ROOT_DIR/crates/platform_winit/src/lib.rs"

# Renderer: worldsmithing placement + HUD wiring
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/mod.rs"
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/foliage.rs"
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/foliage_stream.rs"
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/rocks.rs"
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/renderer/render.rs"
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/renderer/init.rs"
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/renderer/input.rs"

# UI/HUD modules
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/ui/mod.rs"
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/ui/legacy.rs"
add_file "$ROOT_DIR/crates/render_wgpu/src/gfx/ui/hotbar.rs"
add_file "$ROOT_DIR/crates/ux_hud/src/lib.rs"

# Data: zone manifest used by Campaign Builder
add_file "$ROOT_DIR/data/zones/campaign_builder/manifest.json"
add_file "$ROOT_DIR/crates/data_runtime/src/zone.rs"

# 3) Dynamic search (fallback)
if command -v rg >/dev/null 2>&1; then
  rg -n -l -i -g '!**/target/**' -g '!**/.git/**' \
    -e '\\bworldsmithing\\b|\\bhotbar\\b|\\bHUD\\b|\\bkeyboard|KeyCode|logical_key\\b|draw_picker_overlay' \
    "$ROOT_DIR/crates" "$ROOT_DIR/data" 2>/dev/null | while IFS= read -r f; do add_file "$f"; done
else
  grep -R -n -i -E '\\bworldsmithing\\b|\\bhotbar\\b|\\bHUD\\b|\\bkeyboard|KeyCode|logical_key\\b|draw_picker_overlay' \
    "$ROOT_DIR/crates" "$ROOT_DIR/data" 2>/dev/null | cut -d: -f1 | sort -u | while IFS= read -r f; do add_file "$f"; done
fi

# 4) Truncate
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

# 5) Copy
case "$CLIP" in
  pbcopy) pbcopy < "$tmp_all" ;;
  xclip) xclip -selection clipboard < "$tmp_all" ;;
  wl-copy) wl-copy < "$tmp_all" ;;
esac

kb=$(awk -v b="$size" 'BEGIN{printf "%.1f", b/1024}')
echo "Copied ${kb} KiB to clipboard (truncated=${truncated})"

