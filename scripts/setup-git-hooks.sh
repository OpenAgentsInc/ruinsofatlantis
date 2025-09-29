#!/usr/bin/env bash
set -euo pipefail

echo "Configuring repo git hooks to use .githooks ..."
git config core.hooksPath .githooks

echo "Verifying ..."
git config --get core.hooksPath

cat <<'EON'

Done. Pre-push will now run:
  cargo xtask ci   # fmt + clippy -D warnings + WGSL validation + cargo-deny (if installed) + tests + schemas

To skip once (e.g., an urgent CI fix), set:
  RA_SKIP_HOOKS=1 git push

EON

