#!/usr/bin/env bash
set -euo pipefail

# Build a local versioned WASM snapshot for a single zone and
# generate a loader page that mimics the site's /builds/{version} route.
#
# Usage:
#   scripts/build_versioned_local.sh [version] [zone_slug]
#
# Defaults:
#   version   = 0-0-3
#   zone_slug = campaign_builder
#
# Output layout (under dist/):
#   dist/builds/0-0-3/index.html            # local versioned loader
#   dist/builds-static/0-0-3/{manifest/js/wasm/assets/packs}
#     - packs/zones only contains the requested zone

APP_REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VER="${1:-0-0-3}"
ZONE="${2:-campaign_builder}"

echo "[1/8] Ensuring wasm toolchain + trunk present"
if ! rustup target list --installed | grep -q "wasm32-unknown-unknown"; then
  rustup target add wasm32-unknown-unknown
fi
if ! command -v trunk >/dev/null 2>&1; then
  cargo install trunk
fi

echo "[2/8] Baking zone snapshot: $ZONE"
(cd "$APP_REPO_ROOT" && cargo xtask bake-zone -- "$ZONE")

echo "[3/8] Building WASM bundle via trunk --release with ROA_ZONE_DEFAULT=$ZONE"
REBUILD="${REBUILD:-0}"
if [[ "$REBUILD" == "1" ]]; then
  echo "  REBUILD=1 → forcing trunk build"
  (cd "$APP_REPO_ROOT" && ROA_ZONE_DEFAULT="$ZONE" trunk build --release)
else
  if compgen -G "$APP_REPO_ROOT/dist/ruinsofatlantis-*.js" > /dev/null && \
     compgen -G "$APP_REPO_ROOT/dist/ruinsofatlantis-*_bg.wasm" > /dev/null; then
    echo "  Found existing hashed JS/WASM in dist/, skipping trunk build (set REBUILD=1 to force)"
  else
    (cd "$APP_REPO_ROOT" && ROA_ZONE_DEFAULT="$ZONE" trunk build --release)
  fi
fi

DIST_DIR="$APP_REPO_ROOT/dist"
if [[ ! -d "$DIST_DIR" ]]; then
  echo "error: dist/ missing after build" >&2
  exit 1
fi

echo "[4/8] Pruning packs to keep only the requested zone: $ZONE"
if [[ -d "$DIST_DIR/packs/zones" ]]; then
  shopt -s nullglob
  for d in "$DIST_DIR"/packs/zones/*; do
    base="$(basename "$d")"
    if [[ "$base" != "$ZONE" ]]; then
      rm -rf "$d"
    fi
  done
  shopt -u nullglob
fi

echo "[5/8] Locating hashed artifacts in dist/"
MOD_JS="$(basename "$(ls -1 "$DIST_DIR"/ruinsofatlantis-*.js | head -n1)")"
WASM_BIN="$(basename "$(ls -1 "$DIST_DIR"/ruinsofatlantis-*_bg.wasm | head -n1)")"
if [[ -z "$MOD_JS" || -z "$WASM_BIN" ]]; then
  echo "error: could not find hashed JS/WASM in dist/" >&2
  exit 1
fi
echo "  module: $MOD_JS"
echo "  wasm:   $WASM_BIN"

echo "[6/8] Creating local versioned snapshot under dist/builds-static/$VER"
BASE_STATIC="$DIST_DIR/builds-static/$VER"
mkdir -p "$BASE_STATIC/assets" "$BASE_STATIC/packs"
cp -f "$DIST_DIR/$MOD_JS" "$BASE_STATIC/$MOD_JS"
cp -f "$DIST_DIR/$WASM_BIN" "$BASE_STATIC/$WASM_BIN"
if [[ "${SLIM:-0}" == "1" ]]; then
  echo "  SLIM=1: not copying full assets directory"
  if [[ -n "${ASSETS_WHITELIST:-}" ]]; then
    echo "  copying whitelisted assets: $ASSETS_WHITELIST"
    IFS=':' read -r -a items <<< "$ASSETS_WHITELIST"
    for it in "${items[@]}"; do
      src="$DIST_DIR/assets/$it"
      dst_dir="$BASE_STATIC/assets/$(dirname "$it")"
      mkdir -p "$dst_dir"
      if [[ -d "$src" ]]; then
        rsync -a "$src/" "$BASE_STATIC/assets/$it/"
      elif [[ -f "$src" ]]; then
        cp -f "$src" "$BASE_STATIC/assets/$it"
      fi
    done
  fi
else
  rsync -a --delete "$DIST_DIR/assets/" "$BASE_STATIC/assets/"
fi
rsync -a --delete "$DIST_DIR/packs/"  "$BASE_STATIC/packs/"
cat > "$BASE_STATIC/manifest.json" << JSON
{ "mod": "$MOD_JS", "wasm": "$WASM_BIN", "assetsBase": "assets/", "packsBase": "packs/" }
JSON

echo "[7/8] Writing local loader at dist/builds/$VER/index.html"
BASE_ROUTE="$DIST_DIR/builds/$VER"
mkdir -p "$BASE_ROUTE"
cat > "$BASE_ROUTE/index.html" << 'HTML'
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Ruins of Atlantis — Versioned Loader (Local)</title>
    <style>
      html, body { margin: 0; height: 100%; background: #000; }
      body { display: flex; align-items: center; justify-content: center; }
      #overlay { position: fixed; inset: 0; display: flex; align-items: center; justify-content: center; color: #fff; font: 14px/1.4 system-ui, -apple-system, Segoe UI, Roboto, Helvetica, Arial, sans-serif; background: #000; }
      canvas { display: block; width: 100vw; height: 100vh; touch-action: none; }
    </style>
  </head>
  <body>
    <div id="overlay">Loading…</div>
    <script>
      (async () => {
        // Resolve version from path: /builds/<ver>/ => use same <ver> under /builds-static
        const parts = location.pathname.split('/').filter(Boolean);
        const idx = parts.indexOf('builds');
        if (idx < 0 || idx + 1 >= parts.length) {
          document.getElementById('overlay').textContent = 'Invalid version route';
          return;
        }
        const ver = parts[idx + 1];
        const staticBase = `/builds-static/${ver}`;

        // Wrap fetch to rewrite absolute /assets and /packs to the versioned base
        const origFetch = window.fetch.bind(window);
        window.fetch = (input, init) => {
          try {
            const req = (input instanceof Request) ? input : new Request(input, init);
            const u = new URL(req.url, location.origin);
            if (u.origin === location.origin && (u.pathname.startsWith('/assets/') || u.pathname.startsWith('/packs/'))) {
              const rewritten = staticBase + u.pathname;
              const newReq = new Request(rewritten, req);
              return origFetch(newReq);
            }
            return origFetch(req);
          } catch (e) {
            return origFetch(input, init);
          }
        };

        // Load manifest, import module, and init wasm
        const man = await (await fetch(`${staticBase}/manifest.json`)).json();
        const modPath = `${staticBase}/${man.mod}`;
        const wasmPath = `${staticBase}/${man.wasm}`;
        try {
          const mod = await import(modPath);
          await mod.default({ module_or_path: wasmPath });
        } catch (e) {
          console.error('init failed', e);
        } finally {
          const el = document.getElementById('overlay');
          if (el) el.style.display = 'none';
        }
      })();
    </script>
  </body>
</html>
HTML

echo "[8/8] Done"
echo
echo "Local test instructions:"
echo "  1) Serve 'dist/' via a static server (Rust preferred):"
echo "     cargo install basic-http-server # once"
echo "     basic-http-server dist -a 127.0.0.1:8080"
echo "     # or: python3 -m http.server --directory dist 8080"
echo "  2) Open: http://127.0.0.1:8080/builds/$VER/"
echo "     This boots the compiled default zone and rewrites assets/packs to builds-static/$VER."
