#!/usr/bin/env python3
"""
Rebuild the SRD Markdown files directly from the SRD 5.2.1 PDF.

This script uses pdftotext to extract the entire PDF, then slices the text
into chunks that correspond to each Markdown file under docs/srd/.
Existing files are overwritten with verbatim text from the PDF (plus a
Source comment indicating the page range).

WARNING: This will clobber manual edits inside docs/srd/*.md (except README.md).
Review the resulting diffs carefully.
"""
from __future__ import annotations

import bisect
import re
import subprocess
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
SRD_DIR = REPO_ROOT / "docs" / "srd"
PDF_PATH = SRD_DIR / "SRD_CC_v5.2.1.pdf"
TMP_DIR = SRD_DIR / ".tmp"
RAW_TXT = TMP_DIR / "all_raw.txt"


def ensure_pdf_dump() -> str:
    TMP_DIR.mkdir(parents=True, exist_ok=True)
    if not RAW_TXT.exists():
        subprocess.run(
            ["pdftotext", str(PDF_PATH), str(RAW_TXT)],
            check=True,
        )
    return RAW_TXT.read_text(encoding="utf-8")


def collect_ordered_files() -> list[Path]:
    base = SRD_DIR
    ordered: list[Path] = []

    def add_paths(folder: str, names: list[str]) -> None:
        root = base / folder
        for name in names:
            path = root / name
            if not path.exists():
                raise FileNotFoundError(path)
            ordered.append(path)

    add_paths(
        "00-front-matter",
        ["legal-information.md", "contents.md"],
    )

    add_paths(
        "01-playing-the-game",
        [
            "rhythm-of-play.md",
            "six-abilities.md",
            "d20-tests.md",
            "heroic-inspiration.md",
            "proficiency.md",
            "actions.md",
            "social-interaction.md",
            "exploration.md",
            "combat.md",
        ],
    )

    add_paths(
        "02-classes",
        [
            "barbarian.md",
            "bard.md",
            "cleric.md",
            "druid.md",
            "fighter.md",
            "monk.md",
            "paladin.md",
            "ranger.md",
            "rogue.md",
            "sorcerer.md",
            "warlock.md",
            "wizard.md",
        ],
    )

    add_paths("03-spells", ["gaining-spells.md", "casting-spells.md"])

    spell_dir = base / "03-spells" / "spell-descriptions"
    for letter_dir in sorted(p for p in spell_dir.iterdir() if p.is_dir()):
        for spell_file in sorted(letter_dir.glob("*.md")):
            if spell_file.name == "README.md":
                continue
            ordered.append(spell_file)

    feat_root = base / "04-feats"
    add_paths("04-feats", ["origin/README.md"])  # placeholder to keep order
    # Remove placeholder later; README shouldn't be overwritten
    ordered.pop()

    for sub in ["origin", "general", "fighting-style", "epic-boon"]:
        sub_dir = feat_root / sub
        for md in sorted(sub_dir.glob("*.md")):
            if md.name == "README.md":
                continue
            ordered.append(md)

    add_paths(
        "05-equipment",
        [
            "coins.md",
            "weapons.md",
            "weapon-properties.md",
            "weapon-mastery-properties.md",
            "armor-and-shields.md",
            "tools.md",
            "adventuring-gear.md",
            "mounts-and-vehicles.md",
            "lifestyle-expenses.md",
            "food-drink-and-lodging.md",
            "hirelings.md",
            "spellcasting-services.md",
            "magic-items.md",
            "crafting-nonmagical-items.md",
            "brewing-potions-of-healing.md",
            "scribing-spell-scrolls.md",
        ],
    )

    add_paths(
        "06-character-creation",
        [
            "choose-a-character-sheet.md",
            "create-your-character.md",
            "step-1-choose-class.md",
            "step-2-determine-origin.md",
            "step-3-ability-scores.md",
            "step-4-alignment.md",
            "step-5-details.md",
            "level-advancement.md",
            "starting-at-higher-levels.md",
            "multiclassing.md",
            "trinkets.md",
        ],
    )

    monsters_root = base / "07-monsters"
    add_paths(
        "07-monsters",
        [
            "stat-block-overview.md",
            "running-a-monster.md",
        ],
    )
    monsters_dir = monsters_root / "a-z"
    for letter_dir in sorted(p for p in monsters_dir.iterdir() if p.is_dir()):
        for md in sorted(letter_dir.glob("*.md")):
            if md.name == "README.md":
                continue
            if md.name == "ALL.md":
                continue
            ordered.append(md)

    animals_dir = base / "08-animals" / "a-z"
    for letter_dir in sorted(p for p in animals_dir.iterdir() if p.is_dir()):
        for md in sorted(letter_dir.glob("*.md")):
            if md.name == "README.md":
                continue
            if md.name == "ALL.md":
                continue
            ordered.append(md)

    ordered.append(base / "09-rules-glossary" / "rules-glossary.md")

    toolbox_dir = base / "10-gameplay-toolbox"
    toolbox_files = sorted(p for p in toolbox_dir.glob("*.md") if p.name != "README.md")
    ordered.extend(toolbox_files)

    # Verify coverage
    existing = {
        p
        for p in base.rglob("*.md")
        if p.name != "README.md"
        and "ALL.md" not in p.name
        and ".tmp" not in p.parts
    }
    if set(ordered) != existing:
        missing = sorted(existing - set(ordered))
        extra = sorted(set(ordered) - existing)
        raise RuntimeError(f"File coverage mismatch. Missing: {missing[:5]} Extra: {extra[:5]}")

    return ordered


def heading_for_file(path: Path) -> str:
    for line in path.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if not stripped:
            continue
        if stripped.startswith("<!--"):
            continue
        if stripped.startswith("#"):
            return stripped.lstrip("# ").strip()
        return stripped
    raise RuntimeError(f"Unable to determine heading for {path}")


def make_page_index(full_text: str) -> tuple[list[int], int]:
    starts: list[int] = []
    pos = 0
    parts = full_text.split("\f")
    for part in parts:
        starts.append(pos)
        pos += len(part) + 1  # include the form feed delimiter
    total = pos
    return starts, total


def offset_to_page(offset: int, page_starts: list[int]) -> int:
    if offset < 0:
        offset = 0
    idx = bisect.bisect_right(page_starts, offset) - 1
    if idx < 0:
        idx = 0
    return idx + 1


def rebuild() -> None:
    full_text = ensure_pdf_dump()
    ordered_files = collect_ordered_files()

    full_text_lower = full_text.lower()
    page_starts, total_len = make_page_index(full_text)

    starts: list[int] = []
    cursor = 0
    for path in ordered_files:
        heading = heading_for_file(path)
        heading_lc = heading.lower()
        if heading_lc == "contents":
            pattern = re.compile(rf"(^|\n)\s*{re.escape(heading_lc)}\b")
        else:
            pattern = re.compile(
                rf"(^|\n)\s*{re.escape(heading_lc)}\b(?!\s*\.+\s*\d)",
            )
        match = pattern.search(full_text_lower, cursor)
        if not match:
            raise RuntimeError(f"Heading '{heading}' not found in PDF text after offset {cursor} (file {path})")
        start = match.start()
        starts.append(start)
        cursor = start + 1

    starts.append(total_len)

    for idx, path in enumerate(ordered_files):
        start = starts[idx]
        end = starts[idx + 1]
        chunk = full_text[start:end].lstrip("\n").rstrip()
        if not chunk:
            raise RuntimeError(f"Empty chunk for {path}")
        start_page = offset_to_page(start, page_starts)
        end_page = offset_to_page(end - 1, page_starts)
        if start_page == end_page:
            comment = f"<!-- Source: docs/srd/SRD_CC_v5.2.1.pdf p.{start_page} -->"
        else:
            comment = f"<!-- Source: docs/srd/SRD_CC_v5.2.1.pdf pp.{start_page}–{end_page} -->"
        content = f"{comment}\n\n{chunk}\n"
        path.write_text(content, encoding="utf-8")
        rel = path.relative_to(REPO_ROOT)
        print(f"Wrote {rel}")


if __name__ == "__main__":
    rebuild()
