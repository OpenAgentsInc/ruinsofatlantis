#!/usr/bin/env python3
"""
Insert PDF source page citations at the top of SRD Markdown files.

This script scans docs/srd/SRD_CC_v5.2.1.pdf via pdftotext, derives which
page (or page range) each Markdown file originates from, and prepends a
`<!-- Source: ... -->` comment when missing.

Usage:
    python scripts/annotate_srd_sources.py [path ...]

With no arguments, every Markdown file under docs/srd/ (excluding README.md)
is processed. If paths are provided, only those files (or directories) are
considered.
"""
from __future__ import annotations

import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

REPO_ROOT = Path(__file__).resolve().parents[1]
SRD_DIR = REPO_ROOT / "docs" / "srd"
PDF_PATH = SRD_DIR / "SRD_CC_v5.2.1.pdf"
TMP_DIR = SRD_DIR / ".tmp"
RAW_TXT = TMP_DIR / "all_raw.txt"


def ensure_pdf_dump() -> None:
    TMP_DIR.mkdir(parents=True, exist_ok=True)
    if RAW_TXT.exists():
        return
    subprocess.run(
        ["pdftotext", "-layout", str(PDF_PATH), str(RAW_TXT)],
        check=True,
    )


def preprocess_page(page: str) -> str:
    page = page.replace("\r", "")
    # Join hyphenated breaks
    page = re.sub(r"([A-Za-z])-\n\s*([A-Za-z])", r"\1\2", page)
    page = page.lower()
    page = page.replace("\n", " ")
    # Normalize punctuation to spaces
    page = re.sub(r"[^0-9a-z]+", " ", page)
    page = re.sub(r"\s+", " ", page)
    return page.strip()


def preprocess_md(text: str) -> list[str]:
    # Strip leading source comments if present
    text = re.sub(r"^<!--.*?-->\s*", "", text, flags=re.S)
    # Remove code fences to avoid matching formatting artifacts
    text = re.sub(r"```.*?```", "", text, flags=re.S)
    # Replace Markdown links with link text
    text = re.sub(r"\[([^\]]+)\]\([^)]+\)", r"\1", text)
    # Remove HTML comments that might be embedded later
    text = re.sub(r"<!--.*?-->", "", text, flags=re.S)
    text = text.replace("\r", "")
    text = text.lower()
    # Remove Markdown markers
    text = re.sub(r"[#*`>|_~]", " ", text)
    text = re.sub(r"\[|\]", " ", text)
    text = re.sub(r"\(|\)", " ", text)
    text = re.sub(r"[^0-9a-z]+", " ", text)
    text = re.sub(r"\s+", " ", text)
    words = text.strip().split()
    return words


def pick_tokens(words: list[str], reverse: bool = False) -> list[str]:
    if reverse:
        seq = list(reversed(words))
    else:
        seq = words
    tokens: list[str] = []
    for w in seq:
        if len(w) < 2:
            continue
        tokens.append(w)
        if len(tokens) >= 25:
            break
    if reverse:
        tokens = list(reversed(tokens))
    return tokens


@dataclass
class PageIndex:
    pages: list[str]

    def find_forward(self, tokens: Iterable[str], start: int = 0) -> int | None:
        tokens = [t for t in tokens if t]
        if not tokens:
            return None
        for idx in range(start, len(self.pages)):
            page = self.pages[idx]
            if all(t in page for t in tokens):
                return idx
        return None

    def find_backward(self, tokens: Iterable[str], start: int) -> int | None:
        tokens = [t for t in tokens if t]
        if not tokens:
            return None
        for idx in range(start, -1, -1):
            page = self.pages[idx]
            if all(t in page for t in tokens):
                return idx
        return None


def build_index() -> PageIndex:
    ensure_pdf_dump()
    raw = RAW_TXT.read_text(encoding="utf-8")
    # Split by form feed; filter empty tail
    pages = [p for p in raw.split("\f") if p.strip()]
    processed = [preprocess_page(p) for p in pages]
    return PageIndex(processed)


def enumerate_md_files(paths: list[Path]) -> list[Path]:
    targets: list[Path] = []
    if not paths:
        paths = [SRD_DIR]
    for entry in paths:
        entry = entry.resolve()
        if entry.is_dir():
            for md in entry.rglob("*.md"):
                if md.name == "README.md":
                    continue
                targets.append(md)
        elif entry.is_file() and entry.suffix == ".md":
            if entry.name != "README.md":
                targets.append(entry)
    return sorted(set(targets))


def already_annotated(text: str) -> bool:
    stripped = text.lstrip()
    return stripped.startswith("<!-- Source:")


def annotate_file(path: Path, index: PageIndex) -> bool:
    text = path.read_text(encoding="utf-8")
    if already_annotated(text):
        return False

    words = preprocess_md(text)
    if len(words) < 8:
        raise RuntimeError(f"Not enough textual content to locate page range: {path}")

    start_tokens = pick_tokens(words, reverse=False)
    end_tokens = pick_tokens(words, reverse=True)

    start_page = index.find_forward(start_tokens, start=0)
    if start_page is None:
        raise RuntimeError(f"Start tokens not found for {path}")

    end_page = index.find_forward(end_tokens, start=start_page)
    if end_page is None:
        # In rare cases, the tail tokens might wrap to previous page; retry backward search
        end_page = index.find_backward(end_tokens, start=len(index.pages) - 1)
    if end_page is None or end_page < start_page:
        end_page = start_page

    if start_page == end_page:
        comment = f"<!-- Source: docs/srd/SRD_CC_v5.2.1.pdf p.{start_page + 1} -->"
    else:
        comment = f"<!-- Source: docs/srd/SRD_CC_v5.2.1.pdf pp.{start_page + 1}–{end_page + 1} -->"

    new_text = f"{comment}\n\n{text.lstrip()}"
    path.write_text(new_text, encoding="utf-8")
    return True


def main(argv: list[str]) -> None:
    targets = enumerate_md_files([Path(arg) for arg in argv])
    if not targets:
        print("No Markdown files found to annotate.", file=sys.stderr)
        sys.exit(1)

    index = build_index()
    updated = 0
    for path in targets:
        try:
            if annotate_file(path, index):
                updated += 1
                rel = path.relative_to(REPO_ROOT)
                print(f"Annotated {rel}")
        except Exception as exc:  # noqa: BLE001
            rel = path.relative_to(REPO_ROOT)
            print(f"Failed to annotate {rel}: {exc}", file=sys.stderr)

    print(f"Done. Updated {updated} file(s).")


if __name__ == "__main__":
    main(sys.argv[1:])
