#!/usr/bin/env bash
set -euo pipefail

# Copy PNG frames from a Horde repo checkout into our assets folder for the octahedral demo.
# Usage:
#   scripts/fetch_horde_octa_assets.sh /path/to/horde/assets/octa/albedo
# or set HORDE_ASSETS=/path/... and run with no args.

SRC=${1:-${HORDE_ASSETS:-}}
if [[ -z "${SRC}" ]]; then
  echo "Usage: $0 /path/to/horde/albedo_frames_dir  (or set HORDE_ASSETS)" >&2
  exit 1
fi
if [[ ! -d "${SRC}" ]]; then
  echo "Source directory does not exist: ${SRC}" >&2
  exit 1
fi

DEST="$(dirname "$0")/../assets/horde/octa_demo/albedo"
mkdir -p "${DEST}"

echo "Copying PNGs from ${SRC} -> ${DEST} ..."
shopt -s nullglob
count=0
for f in "${SRC}"/*.png "${SRC}"/*.PNG; do
  cp -f "$f" "${DEST}/"
  count=$((count+1))
done
echo "Copied ${count} PNG files."

echo "Done. Launch with: RA_ZONE=octa_demo cargo run"
