# Asset Pipeline & Git LFS — 2025-10-04

Status
- `.gitattributes` tracks `assets/models/**` and `assets/anims/**` via LFS (evidence/gitattributes.txt).
- Absolute paths found in docs/scripts as examples (evidence/absolute-paths.txt) — prefer relative paths or env var placeholders.

Findings
- F-ASSET-007: Extend LFS patterns to cover textures and binary buffers commonly used by GLTF (P2 Low).
- F-DOCS-008: Absolute paths in docs/scripts (P3 Low).

Recommendations
- Add LFS rules for: `*.glb`, `*.gltf` (already under models), `*.bin`, `*.png`, `*.jpg`, `*.jpeg`, `*.ktx2` as applicable to runtime/tools.
- Avoid embedding absolute user paths in docs; show with `$HOME/...` or use placeholders. In scripts, default via env (`SITE_REPO`) is already present — keep it the only absolute default.

