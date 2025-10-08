#!/usr/bin/env bash
set -euo pipefail

echo "Configuring repo git hooks to use .githooks ..."
git config core.hooksPath .githooks

echo "Verifying ..."
git config --get core.hooksPath

cat <<'EON'

Done. Pre-push will now run:
  cargo xtask ci   # fmt + clippy -D warnings + WGSL validation + cargo-deny (if installed) + tests + schemas

Pre-commit will auto-run cargo fmt and stage formatting changes so your commit
always includes formatted code. You can bypass hooks with --no-verify or:
  RA_SKIP_HOOKS=1 git commit / git push

To skip once (e.g., an urgent CI fix), set:
  RA_SKIP_HOOKS=1 git push

EON
