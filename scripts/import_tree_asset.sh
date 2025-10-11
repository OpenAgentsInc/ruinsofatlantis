#!/usr/bin/env bash
set -euo pipefail

# Import a tree model into assets/trees/ and ensure it's tracked by git‑lfs.
# Usage:
#   scripts/import_tree_asset.sh /absolute/or/relative/path/to/Birch_4GLB.glb [dest_name.glb]

SRC=${1:-}
DEST_NAME=${2:-}

if [[ -z "${SRC}" ]]; then
  echo "Usage: $0 /path/to/SomeTree.glb [dest_name.glb]" >&2
  exit 1
fi

if [[ ! -f "${SRC}" ]]; then
  echo "error: source file not found: ${SRC}" >&2
  exit 2
fi

mkdir -p assets/trees

if [[ -z "${DEST_NAME}" ]]; then
  DEST_NAME=$(basename "${SRC}")
fi

DEST="assets/trees/${DEST_NAME}"
cp "${SRC}" "${DEST}"

# Ensure .glb files are tracked by LFS
if ! grep -q 'assets/**/*.glb' .gitattributes; then
  echo 'assets/**/*.glb  filter=lfs diff=lfs merge=lfs -text' >> .gitattributes
fi

echo "Imported ${SRC} -> ${DEST}"
echo "Run: git lfs track 'assets/**/*.glb' && git add .gitattributes '${DEST}' && git commit -m 'assets: add tree model'"

