#!/usr/bin/env python3
"""
Extract key sections from docs/srd/SRD_CC_v5.2.1.pdf into Markdown files.

Currently handled:
- Rules Glossary (full text to docs/srd/09-rules-glossary/rules-glossary.md)
- Gameplay Toolbox topics split into individual files under docs/srd/10-gameplay-toolbox/

This script expects pdftotext to be available on PATH. It writes an intermediate
plain-text dump to docs/srd/.tmp/all.txt if not present and reuses it on reruns.
"""
from __future__ import annotations

import os
import re
import subprocess
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
SRD_DIR = REPO_ROOT / "docs" / "srd"
PDF_PATH = SRD_DIR / "SRD_CC_v5.2.1.pdf"
TMP_DIR = SRD_DIR / ".tmp"
TXT_ALL = TMP_DIR / "all.txt"


def run(cmd: list[str]) -> str:
    res = subprocess.run(cmd, capture_output=True, text=True, check=True)
    return res.stdout


def ensure_text_dump() -> str:
    TMP_DIR.mkdir(parents=True, exist_ok=True)
    if not TXT_ALL.exists():
        run(["pdftotext", "-layout", str(PDF_PATH), str(TXT_ALL)])
    return TXT_ALL.read_text(encoding="utf-8", errors="ignore")


def sanitize(text: str) -> str:
    # Remove form feeds and trailing spaces
    text = text.replace("\r", "").replace("\f", "\n")
    # Drop footer lines like "178   System Reference Document 5.2.1"
    text = re.sub(r"^\s*\d+\s+System Reference Document 5\.2\.1\s*$", "", text, flags=re.M)
    # Collapse multiple blank lines
    text = re.sub(r"\n{3,}", "\n\n", text)
    # Fix common hyphenation where a word breaks with a hyphen at line end
    # e.g., "mod-\n ifier" -> "modifier"
    text = re.sub(r"([A-Za-z])\-\n\s*([a-z])", r"\1\2", text)
    # Join lines that are clearly a wrapped sentence (heuristic)
    text = re.sub(r"([^\n])\n(?!\n|\#|\-|\*|\d+\.|\s{2,})", r" \1\n", text)
    return text.strip() + "\n"


def slice_between(all_text: str, start_pat: str, end_pat: str) -> str:
    ms = re.search(start_pat, all_text, flags=re.M)
    if not ms:
        raise RuntimeError(f"Start pattern not found: {start_pat}")
    me = re.search(end_pat, all_text, flags=re.M)
    if not me:
        raise RuntimeError(f"End pattern not found: {end_pat}")
    return all_text[ms.start():me.start()]


def write_rules_glossary(all_text: str) -> None:
    out_dir = SRD_DIR / "09-rules-glossary"
    out_dir.mkdir(parents=True, exist_ok=True)
    seg = slice_between(all_text, r"^\s*Rules Glossary\b", r"^\s*Gameplay Toolbox\b")
    seg = sanitize(seg)
    (out_dir / "README.md").write_text(
        """# Rules Glossary

This folder contains the SRD 5.2.1 Rules Glossary in Markdown form. For now, the content is provided as a single file mirroring the SRD text; we can split by term later if desired.

- Full Glossary — docs/srd/09-rules-glossary/rules-glossary.md
""",
        encoding="utf-8",
    )
    (out_dir / "rules-glossary.md").write_text(
        """<!-- Source: docs/srd/SRD_CC_v5.2.1.pdf (Rules Glossary, pp. ~176–191) -->

# Rules Glossary

"""
        + seg,
        encoding="utf-8",
    )


def write_gameplay_toolbox(all_text: str) -> None:
    out_dir = SRD_DIR / "10-gameplay-toolbox"
    out_dir.mkdir(parents=True, exist_ok=True)
    seg = slice_between(all_text, r"^\s*Gameplay Toolbox\b", r"^\s*Monsters\b")
    seg = sanitize(seg)

    topics = [
        ("travel-pace", r"^.*Travel Pace.*$"),
        ("creating-a-background", r"^.*Creating a Background.*$"),
        ("curses-and-magical-contagions", r"^.*Curses and Magical Contagions.*$"),
        ("environmental-effects", r"^.*Environmental Effects.*$"),
        ("fear-and-mental-stress", r"^.*Fear and Mental Stress.*$"),
        ("poison", r"^.*Poison.*$"),
        ("traps", r"^.*Traps.*$"),
        ("combat-encounters", r"^.*Combat Encounters.*$"),
        ("magic-items", r"^.*Magic Items.*$"),
    ]

    # Find indices of each topic heading within seg
    indices: list[tuple[str, int]] = []
    for slug, pat in topics:
        m = re.search(pat, seg, flags=re.M)
        if m:
            indices.append((slug, m.start()))
    # Sort by position
    indices.sort(key=lambda x: x[1])
    if not indices:
        raise RuntimeError("No Gameplay Toolbox headings found.")

    # Slice segments between headings
    pieces: list[tuple[str, str]] = []
    for i, (slug, start) in enumerate(indices):
        end = indices[i + 1][1] if i + 1 < len(indices) else len(seg)
        chunk = seg[start:end].strip()
        pieces.append((slug, chunk))

    # Write files
    (out_dir / "README.md").write_text(
        """# Gameplay Toolbox

Rules and procedures for overland travel, background creation, hazards, traps, encounters, and magic item usage.

"""
        + "\n".join(f"- {title_from_slug(slug)} — docs/srd/10-gameplay-toolbox/{slug}.md" for slug, _ in pieces)
        + "\n",
        encoding="utf-8",
    )

    for slug, chunk in pieces:
        title = title_from_slug(slug)
        (out_dir / f"{slug}.md").write_text(
            f"""<!-- Source: docs/srd/SRD_CC_v5.2.1.pdf (Gameplay Toolbox) -->

# {title}

"""
            + chunk
            + "\n",
            encoding="utf-8",
        )


def title_from_slug(slug: str) -> str:
    return slug.replace("-", " ").title()


def main() -> None:
    all_text = ensure_text_dump()
    write_rules_glossary(all_text)
    write_gameplay_toolbox(all_text)
    # Also dump aggregate Monsters A–Z and Animals for reference
    mon_dir = SRD_DIR / "07-monsters" / "a-z"
    mon_dir.mkdir(parents=True, exist_ok=True)
    mon_seg = slice_between(all_text, r"^\s*Monsters A–Z\b", r"^\s*Animals\b")
    (mon_dir / "ALL.md").write_text(
        """<!-- Source: docs/srd/SRD_CC_v5.2.1.pdf (Monsters A–Z) -->

# Monsters A–Z (Aggregate)

""" + sanitize(mon_seg),
        encoding="utf-8",
    )

    ani_dir = SRD_DIR / "08-animals" / "a-z"
    ani_dir.mkdir(parents=True, exist_ok=True)
    ani_seg = all_text[ re.search(r"^\s*Animals\b", all_text, flags=re.M).start() : ]
    (ani_dir / "ALL.md").write_text(
        """<!-- Source: docs/srd/SRD_CC_v5.2.1.pdf (Animals) -->

# Animals (Aggregate)

""" + sanitize(ani_seg),
        encoding="utf-8",
    )
    print("Extracted: Rules Glossary and Gameplay Toolbox")


if __name__ == "__main__":
    main()
