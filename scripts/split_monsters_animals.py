#!/usr/bin/env python3
"""
Heuristic splitter for Monsters and Animals from the aggregate files generated
by extract_srd_sections.py. Writes one creature per file under the A–Z folders.

This is best-effort and may need manual review for edge cases.
"""
from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def slugify(name: str) -> str:
    s = name.lower()
    s = re.sub(r"[’'`]+", "", s)
    s = re.sub(r"[^a-z0-9\-\s()]+", "", s)
    s = s.strip().replace(" ", "-")
    s = re.sub(r"-+", "-", s)
    return s


SIZE_RE = re.compile(r"^(Tiny|Small|Medium|Large|Huge|Gargantuan)\b.*?,", re.M)


def split_creatures(agg_path: Path, out_dir: Path) -> int:
    text = agg_path.read_text(encoding="utf-8")
    lines = text.splitlines()
    # Find candidate start indices
    starts: list[tuple[int, str]] = []
    i = 0
    while i < len(lines):
        raw = lines[i]
        line = raw.strip()
        if not line or line.startswith("#") or line.startswith("<!--"):
            i += 1
            continue
        # Candidate name line: moderately short, title-ish
        if 2 < len(line) < 60 and re.match(r"^[A-Z][A-Za-z0-9’ ()\-]+$", line):
            # Peek ahead to find next non-empty line
            j = i + 1
            while j < len(lines) and not lines[j].strip():
                j += 1
            nxt = lines[j].strip() if j < len(lines) else ""
            # Allow duplicate name line or size line
            if nxt == line or SIZE_RE.match(nxt):
                starts.append((i, line.strip()))
                # Skip ahead a bit to avoid picking the duplicate name line as a new start
                i = j
        i += 1

    # Slice content for each creature
    count = 0
    for idx, (start_i, name) in enumerate(starts):
        end_i = starts[idx + 1][0] if idx + 1 < len(starts) else len(lines)
        block = "\n".join(lines[start_i:end_i]).strip()
        if not block:
            continue
        letter = name[0].upper()
        dest_dir = out_dir / letter
        dest_dir.mkdir(parents=True, exist_ok=True)
        slug = slugify(name)
        out = dest_dir / f"{slug}.md"
        # Prepend heading if missing
        if not block.startswith("# "):
            block = f"# {name}\n\n" + block
        out.write_text(block + "\n", encoding="utf-8")
        count += 1
    return count


def main() -> None:
    mon_agg = ROOT / "docs" / "srd" / "07-monsters" / "a-z" / "ALL.md"
    ani_agg = ROOT / "docs" / "srd" / "08-animals" / "a-z" / "ALL.md"
    if mon_agg.exists():
        out_m = mon_agg.parent
        n = split_creatures(mon_agg, out_m)
        print(f"Split Monsters: {n} files written under {out_m}")
    else:
        print("Monsters aggregate not found; run extract_srd_sections.py first.")
    if ani_agg.exists():
        out_a = ani_agg.parent
        n = split_creatures(ani_agg, out_a)
        print(f"Split Animals: {n} files written under {out_a}")
    else:
        print("Animals aggregate not found; run extract_srd_sections.py first.")


if __name__ == "__main__":
    main()
