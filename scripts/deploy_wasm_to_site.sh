#!/usr/bin/env bash
set -euo pipefail

# Build and deploy the WASM bundle to the ruinsofatlantis.com Laravel site,
# update the Blade view to point at the new hashed artifacts, then commit,
# push, and open a PR.
#
# Defaults target local dev layout on this machine:
#   - App repo: current working tree (this script lives here)
#   - Site repo: /Users/christopherdavid/code/ruinsofatlantis.com
#   - Public subdir: root (copy JS/WASM at site public/ root; assets/packs under public/)
#
# Usage:
#   scripts/deploy_wasm_to_site.sh
#   SITE_REPO=/path/to/site scripts/deploy_wasm_to_site.sh
#   RUN_CI=1 scripts/deploy_wasm_to_site.sh   # run cargo xtask ci first
#   NO_PR=1 scripts/deploy_wasm_to_site.sh    # skip creating a PR
#
# Requirements:
#   - rustup target add wasm32-unknown-unknown
#   - trunk installed (cargo install trunk)
#   - gh CLI installed and authenticated for PR creation

APP_REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SITE_REPO="${SITE_REPO:-/Users/christopherdavid/code/ruinsofatlantis.com}"
PUBLIC_SUBDIR="${PUBLIC_SUBDIR:-}"        # e.g., "wasm" to copy under public/wasm
RUN_CI="${RUN_CI:-0}"
NO_PR="${NO_PR:-0}"

if [[ ! -d "$SITE_REPO/.git" ]]; then
  echo "error: SITE_REPO does not look like a git repo: $SITE_REPO" >&2
  exit 1
fi

echo "[1/7] Ensuring wasm toolchain + trunk present"
if ! rustup target list --installed | grep -q "wasm32-unknown-unknown"; then
  rustup target add wasm32-unknown-unknown
fi
if ! command -v trunk >/dev/null 2>&1; then
  cargo install trunk
fi

if [[ "$RUN_CI" == "1" ]]; then
  echo "[2/7] Running workspace CI (fmt+clippy+wgsl+tests+schemas)"
  (cd "$APP_REPO_ROOT" && cargo xtask ci)
else
  echo "[2/7] Skipping CI (set RUN_CI=1 to enable)"
fi

echo "[3/7] Building WASM bundle via trunk --release"
(cd "$APP_REPO_ROOT" && trunk build --release)

echo "[4/7] Locating hashed artifacts in dist/"
DIST_DIR="$APP_REPO_ROOT/dist"
MOD_JS="$(basename "$(ls -1 "$DIST_DIR"/ruinsofatlantis-*.js | head -n1)")"
WASM_BIN="$(basename "$(ls -1 "$DIST_DIR"/ruinsofatlantis-*_bg.wasm | head -n1)")"
if [[ -z "$MOD_JS" || -z "$WASM_BIN" ]]; then
  echo "error: could not find hashed JS/WASM in dist/" >&2
  exit 1
fi
echo "  module: $MOD_JS"
echo "  wasm:   $WASM_BIN"

echo "[5/7] Preparing site repo branch"
DATE_TAG="$(date +%Y%m%d-%H%M%S)"
BRANCH="wasm/deploy-${DATE_TAG}"

(
  cd "$SITE_REPO"
  git fetch origin
  # Stash local changes, update main, and create branch BEFORE copying artifacts
  if [[ -n "$(git status --porcelain)" ]]; then
    echo "site repo: stashing local changes"
    git stash push -u -m "temp wasm deploy changes" >/dev/null 2>&1 || true
  fi
  git checkout main
  git pull --ff-only origin main
  git checkout -b "$BRANCH"
)

echo "[6/7] Copying assets to site public directory (on branch $BRANCH)"
DEST_PUBLIC="$SITE_REPO/public"
DEST_DIR="$DEST_PUBLIC"
if [[ -n "$PUBLIC_SUBDIR" ]]; then
  DEST_DIR="$DEST_PUBLIC/$PUBLIC_SUBDIR"
fi

mkdir -p "$DEST_DIR"

# Sync assets/ and packs/ directories (delete removed files)
rsync -av --delete "$DIST_DIR/assets/" "$DEST_PUBLIC/assets/"
rsync -av --delete "$DIST_DIR/packs/"  "$DEST_PUBLIC/packs/"

# Remove old hashed JS/WASM at the chosen destination root (default public/ root)
find "$DEST_DIR" -maxdepth 1 -type f -name 'ruinsofatlantis-*.js' -delete || true
find "$DEST_DIR" -maxdepth 1 -type f -name 'ruinsofatlantis-*_bg.wasm' -delete || true

# Copy new JS/WASM
cp -v "$DIST_DIR/$MOD_JS" "$DEST_DIR/$MOD_JS"
cp -v "$DIST_DIR/$WASM_BIN" "$DEST_DIR/$WASM_BIN"

echo "[7/7] Updating Blade view and pushing PR"
PLAY_BLADE="$SITE_REPO/resources/views/play.blade.php"
if [[ ! -f "$PLAY_BLADE" ]]; then
  echo "error: Blade view not found: $PLAY_BLADE" >&2
  exit 1
fi

if [[ -n "$PUBLIC_SUBDIR" ]]; then
  MOD_PATH_REPL="/${PUBLIC_SUBDIR}/${MOD_JS}"
  WASM_PATH_REPL="/${PUBLIC_SUBDIR}/${WASM_BIN}"
else
  MOD_PATH_REPL="/${MOD_JS}"
  WASM_PATH_REPL="/${WASM_BIN}"
fi

# Replace the const modPath / wasmPath lines using BSD-compatible sed
tmpfile="$(mktemp)"
sed -E \
  -e "s|^([[:space:]]*const[[:space:]]+modPath[[:space:]]*=)[[:space:]]*'.*';|\\1 '${MOD_PATH_REPL}';|" \
  -e "s|^([[:space:]]*const[[:space:]]+wasmPath[[:space:]]*=)[[:space:]]*'.*';|\\1 '${WASM_PATH_REPL}';|" \
  "$PLAY_BLADE" > "$tmpfile"
mv "$tmpfile" "$PLAY_BLADE"

(
  cd "$SITE_REPO"
  # Stage only expected paths
  if [[ -n "$PUBLIC_SUBDIR" ]]; then
    JS_GLOB="public/${PUBLIC_SUBDIR}/ruinsofatlantis-*.js"
    WASM_GLOB="public/${PUBLIC_SUBDIR}/ruinsofatlantis-*_bg.wasm"
  else
    JS_GLOB="public/ruinsofatlantis-*.js"
    WASM_GLOB="public/ruinsofatlantis-*_bg.wasm"
  fi
  git add -A public/assets/ public/packs/ "$JS_GLOB" "$WASM_GLOB" resources/views/play.blade.php 2>/dev/null || true
  COMMIT_MSG="site: deploy latest wasm bundle (${MOD_JS})"
  if git commit -m "$COMMIT_MSG"; then
    git push -u origin "$BRANCH"
    if [[ "$NO_PR" != "1" ]]; then
      PR_TITLE="site/wasm: update bundle to ${MOD_JS}"
      PR_BODY=$(cat <<'PR'
This PR updates the deployed WebGPU WASM bundle:

- Copies latest `assets/` and `packs/` into `public/`
- Publishes new hashed artifacts (`ruinsofatlantis-*.js` and `*_bg.wasm`) at the public root
- Updates `resources/views/play.blade.php` to reference the new hashed filenames

Notes
- Built via `trunk build --release`
- Artifacts remain at the public root to match the Blade loader’s absolute paths
PR
)
      if command -v gh >/dev/null 2>&1; then
        gh pr create -t "$PR_TITLE" -b "$PR_BODY" -B main || echo "warning: failed to open PR via gh"
      else
        echo "gh not found; skipping PR creation."
      fi
    else
      echo "NO_PR=1 set; skipping PR creation."
    fi
  else
    echo "No changes to commit; deployment already current."
  fi
)

echo "Done. Deployed JS: $MOD_JS; WASM: $WASM_BIN"
