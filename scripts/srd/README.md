# SRD Conversion Scripts

Helper scripts to accelerate converting SRD 5.2.1 PDF to organized Markdown.

- `extract.sh` — runs `pdftotext` to generate text chunks for Spells, Monsters, and Animals into `docs/srd/.tmp/`.
- `parse_monsters.py` — heuristically splits the Monsters text into per-creature Markdown files under `docs/srd/07-monsters/a-z/`. Outputs raw text blocks for manual review.

Usage:

- bash scripts/srd/extract.sh
- python3 scripts/srd/parse_monsters.py docs/srd/.tmp/monsters_254_343.txt

Notes:

- Page ranges are approximate and may need adjustment if the SRD PDF layout changes.
- The parser is conservative and won’t overwrite existing files.
- After generation, manually review each monster file and convert the raw fenced block to a cleaned stat block using `docs/srd/07-monsters/TEMPLATE.stat-block.md`.

