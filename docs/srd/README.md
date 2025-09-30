# D&D 5E SRD 5.2.1 — Markdown Conversion

This folder contains a clean, navigable Markdown conversion of the System Reference Document 5.2.1 (SRD 5.2.1).

Attribution (required by CC‑BY‑4.0):

> This work includes material from the System Reference Document 5.2.1 (“SRD 5.2.1”) by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.

See `NOTICE` at the repository root for the canonical attribution statement. Do not add additional Wizards of the Coast branding or logos.

## Goals

- Faithfully reproduce SRD 5.2.1 content in Markdown
- Use a clear, stable file/folder hierarchy with cross‑links
- Make sections easy to diff, review, and reference
- Preserve terminology and rules verbatim; fix only layout/formatting artifacts from PDF extraction

## Folder Structure

- `00-front-matter/`
  - `legal-information.md` (SRD license page)
  - `contents.md` (project contents mirroring this hierarchy)
- `01-playing-the-game/`
  - `README.md` (section index)
  - `rhythm-of-play.md`
  - `six-abilities.md` (incl. Ability Descriptions/Scores/Modifiers)
  - `d20-tests.md` (checks, saves, attack rolls)
  - `heroic-inspiration.md`
  - `proficiency.md`
  - `actions.md` (incl. Bonus Actions and Reactions)
  - `social-interaction.md`
  - `exploration.md` (vision/light, objects, travel)
  - `combat.md`
- `02-classes/`
  - `README.md`
  - `barbarian.md`
  - `bard.md`
  - `cleric.md`
  - `druid.md`
  - `fighter.md`
  - `monk.md`
  - `paladin.md`
  - `ranger.md`
  - `rogue.md`
  - `sorcerer.md`
  - `warlock.md`
  - `wizard.md`
- `03-spells/`
  - `README.md`
  - `gaining-spells.md`
  - `casting-spells.md`
  - `spell-descriptions/README.md`
  - `spell-descriptions/A/README.md`

Names use a numeric prefix to preserve SRD order and kebab‑case file names for stable links.

## Conversion Process

1) Extract text per page
   - Use `pdftotext -layout docs/srd/SRD_CC_v5.2.1.pdf` with `-f/-l` to target pages.
   - Keep a note of PDF page ranges in each Markdown file’s header.

2) Normalize and clean
   - Join words broken by PDF hyphenation (e.g., “partic- ularly” → “particularly”).
   - Reflow paragraphs; remove extra spaces; normalize quotes.
   - Convert tables to Markdown tables; keep order and wording.

3) Structure and link
   - Split content by logical SRD headings into separate files.
   - Use `#`/`##` headings mirroring the SRD.
   - Add relative links between sections (e.g., from checks to proficiency).

4) Verify fidelity
   - Compare against the PDF; avoid paraphrasing or adding rules text.
   - If a diagram/table spans pages, keep it intact in a single file.

5) Licenses and notices
   - Keep the exact CC‑BY‑4.0 attribution (above) and ensure `NOTICE` stays current.
   - If we deviate or add commentary, mark those blocks clearly as “Editor’s Note”.

## Status

- Section 1 (Playing the Game): complete.
- Section 2 (Classes): complete.
- Section 3 (Spells): complete — one file per spell and letter indices.

## Index

- Front Matter
  - docs/srd/00-front-matter/legal-information.md
  - docs/srd/00-front-matter/contents.md
- Playing the Game
  - docs/srd/01-playing-the-game/README.md
  - docs/srd/01-playing-the-game/actions.md
  - docs/srd/01-playing-the-game/combat.md
  - docs/srd/01-playing-the-game/d20-tests.md
  - docs/srd/01-playing-the-game/exploration.md
  - docs/srd/01-playing-the-game/heroic-inspiration.md
  - docs/srd/01-playing-the-game/proficiency.md
  - docs/srd/01-playing-the-game/rhythm-of-play.md
  - docs/srd/01-playing-the-game/six-abilities.md
  - docs/srd/01-playing-the-game/social-interaction.md
- Classes
  - docs/srd/02-classes/README.md
  - docs/srd/02-classes/barbarian.md
  - docs/srd/02-classes/bard.md
  - docs/srd/02-classes/cleric.md
  - docs/srd/02-classes/druid.md
  - docs/srd/02-classes/fighter.md
  - docs/srd/02-classes/monk.md
  - docs/srd/02-classes/paladin.md
  - docs/srd/02-classes/ranger.md
  - docs/srd/02-classes/rogue.md
  - docs/srd/02-classes/sorcerer.md
  - docs/srd/02-classes/warlock.md
  - docs/srd/02-classes/wizard.md
- Spells
  - docs/srd/03-spells/README.md
  - docs/srd/03-spells/gaining-spells.md
  - docs/srd/03-spells/casting-spells.md
  - docs/srd/03-spells/spell-descriptions/README.md
