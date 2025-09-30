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
  - `spell-descriptions/{A,B,C,...}/README.md` (per-letter indexes)
  
- `04-feats/`
  - `README.md`
  - `origin/` (e.g., Alert, Magic Initiate)
  - `general/` (e.g., Ability Score Improvement, Grappler)
  - `fighting-style/` (e.g., Archery, Defense)
  - `epic-boon/` (e.g., Boon of Combat Prowess)

- `05-equipment/`
  - `README.md`
  - `coins.md`, `weapons.md`, `weapon-properties.md`, `weapon-mastery-properties.md`
  - `armor-and-shields.md`, `tools.md`, `adventuring-gear.md`
  - `mounts-and-vehicles.md`, `lifestyle-expenses.md`, `food-drink-and-lodging.md`
  - `hirelings.md`, `spellcasting-services.md`, `magic-items.md`
  - `crafting-nonmagical-items.md`, `brewing-potions-of-healing.md`, `scribing-spell-scrolls.md`

- `06-character-creation/`
  - `README.md`
  - `choose-a-character-sheet.md`
  - `create-your-character.md`
  - `step-1-choose-class.md`
  - `step-2-determine-origin.md`
  - `step-3-ability-scores.md`
  - `step-4-alignment.md`
  - `step-5-details.md`
  - `level-advancement.md`
  - `starting-at-higher-levels.md`
  - `multiclassing.md`
  - `trinkets.md`

Names use a numeric prefix for stable links. Some numbers deviate from the SRD’s print order to minimize churn; see `00-front-matter/contents.md` for the canonical hierarchy.

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

- Playing the Game: complete.
- Classes: all base classes added (with spell lists/subclasses per SRD scope).
- Spells: complete.
- Feats: complete (Origin, General, Fighting Style, Epic Boon).
- Equipment: complete (tables and descriptions transcribed).
- Character Creation: complete (including Trinkets table).
- Rules Glossary: ADDED in 09-rules-glossary/ (full text extracted; further per‑term splits welcome later).
- Gameplay Toolbox: ADDED in 10-gameplay-toolbox/ (topics split into individual files).
- Monsters: complete (A–Z stat blocks transcribed).
- Animals: complete (A–Z stat blocks transcribed).

See Gap Audit below for details on remaining cleanup tasks.

## Index

- Front Matter
  - docs/srd/00-front-matter/legal-information.md
  - docs/srd/00-front-matter/contents.md
- Playing the Game
  - docs/srd/01-playing-the-game/README.md
  - docs/srd/01-playing-the-game/rhythm-of-play.md
  - docs/srd/01-playing-the-game/six-abilities.md
  - docs/srd/01-playing-the-game/d20-tests.md
  - docs/srd/01-playing-the-game/heroic-inspiration.md
  - docs/srd/01-playing-the-game/proficiency.md
  - docs/srd/01-playing-the-game/actions.md
  - docs/srd/01-playing-the-game/social-interaction.md
  - docs/srd/01-playing-the-game/exploration.md
  - docs/srd/01-playing-the-game/combat.md
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
- Feats
  - docs/srd/04-feats/README.md
  - docs/srd/04-feats/origin/README.md
  - docs/srd/04-feats/general/README.md
  - docs/srd/04-feats/fighting-style/README.md
  - docs/srd/04-feats/epic-boon/README.md
- Equipment
  - docs/srd/05-equipment/README.md
  - docs/srd/05-equipment/coins.md
  - docs/srd/05-equipment/weapons.md
  - docs/srd/05-equipment/weapon-properties.md
  - docs/srd/05-equipment/weapon-mastery-properties.md
  - docs/srd/05-equipment/armor-and-shields.md
  - docs/srd/05-equipment/tools.md
  - docs/srd/05-equipment/adventuring-gear.md
  - docs/srd/05-equipment/mounts-and-vehicles.md
  - docs/srd/05-equipment/lifestyle-expenses.md
  - docs/srd/05-equipment/food-drink-and-lodging.md
  - docs/srd/05-equipment/hirelings.md
  - docs/srd/05-equipment/spellcasting-services.md
  - docs/srd/05-equipment/magic-items.md
  - docs/srd/05-equipment/crafting-nonmagical-items.md
  - docs/srd/05-equipment/brewing-potions-of-healing.md
  - docs/srd/05-equipment/scribing-spell-scrolls.md
 - Monsters
  - docs/srd/07-monsters/README.md
  - docs/srd/07-monsters/stat-block-overview.md
  - docs/srd/07-monsters/running-a-monster.md
  - docs/srd/07-monsters/a-z/README.md
    - A — docs/srd/07-monsters/a-z/A/README.md
    - B — docs/srd/07-monsters/a-z/B/README.md
    - C — docs/srd/07-monsters/a-z/C/README.md
    - D — docs/srd/07-monsters/a-z/D/README.md
    - E — docs/srd/07-monsters/a-z/E/README.md
    - F — docs/srd/07-monsters/a-z/F/README.md
    - G — docs/srd/07-monsters/a-z/G/README.md
    - H — docs/srd/07-monsters/a-z/H/README.md
    - I — docs/srd/07-monsters/a-z/I/README.md
    - J — docs/srd/07-monsters/a-z/J/README.md
    - K — docs/srd/07-monsters/a-z/K/README.md
    - L — docs/srd/07-monsters/a-z/L/README.md
    - M — docs/srd/07-monsters/a-z/M/README.md
    - N — docs/srd/07-monsters/a-z/N/README.md
    - O — docs/srd/07-monsters/a-z/O/README.md
    - P — docs/srd/07-monsters/a-z/P/README.md
    - Q — docs/srd/07-monsters/a-z/Q/README.md
    - R — docs/srd/07-monsters/a-z/R/README.md
    - S — docs/srd/07-monsters/a-z/S/README.md
    - T — docs/srd/07-monsters/a-z/T/README.md
    - U — docs/srd/07-monsters/a-z/U/README.md
    - V — docs/srd/07-monsters/a-z/V/README.md
    - W — docs/srd/07-monsters/a-z/W/README.md
    - X — docs/srd/07-monsters/a-z/X/README.md
    - Y — docs/srd/07-monsters/a-z/Y/README.md
    - Z — docs/srd/07-monsters/a-z/Z/README.md
- Animals
  - docs/srd/08-animals/README.md
- Rules Glossary
  - docs/srd/09-rules-glossary/README.md
  - docs/srd/09-rules-glossary/rules-glossary.md
- Gameplay Toolbox
  - docs/srd/10-gameplay-toolbox/README.md
  - docs/srd/10-gameplay-toolbox/travel-pace.md
  - docs/srd/10-gameplay-toolbox/creating-a-background.md
  - docs/srd/10-gameplay-toolbox/curses-and-magical-contagions.md
  - docs/srd/10-gameplay-toolbox/environmental-effects.md
  - docs/srd/10-gameplay-toolbox/fear-and-mental-stress.md
  - docs/srd/10-gameplay-toolbox/poison.md
  - docs/srd/10-gameplay-toolbox/traps.md
  - docs/srd/10-gameplay-toolbox/combat-encounters.md
  - docs/srd/10-gameplay-toolbox/magic-items.md

## Gap Audit

The following discrepancies were found when comparing the PDF’s table of contents to this Markdown conversion:

- Missing sections (now added): Rules Glossary; Gameplay Toolbox (Travel Pace; Creating a Background; Curses and Magical Contagions; Environmental Effects; Fear and Mental Stress; Poison; Traps; Combat Encounters; Magic Items).
- Monsters A–Z: initial normalization complete (one creature per file with per‑letter indexes). Remaining: refine edge cases where adjacent stat blocks were included; ensure one stat block per file and headings/CRs are correct.
- Animals A–Z: initial normalization complete. Remaining: refine edge cases and validate each file’s stat block fields.
- Cross‑links: numerous cross‑references to “Rules Glossary” were dangling. These now resolve via 09-rules-glossary/rules-glossary.md. Future work: link directly to per‑term anchors once the glossary is split per term.
- Adventuring Gear: cleaned and re‑extracted directly from the PDF; review wide table formatting in Markdown viewers.

## Tools

- Extract sections from the PDF (Rules Glossary, Gameplay Toolbox; aggregate Monsters/Animals; Adventuring Gear):
  - `python3 scripts/extract_srd_sections.py`
- Experimental splitter for Monsters/Animals from the aggregate files (manual review recommended):
  - `python3 scripts/split_monsters_animals.py`

## Next Steps

- Monsters/Animals refinement pass
  - Split any residual multi‑creature blocks at the next Size/Type line; ensure exactly one stat block per file.
  - Validate required fields (AC, HP, Speed, Size/Type, Senses, Languages, CR) and trim stray section headers.
  - Update top‑level indexes if creature filenames change; keep letter `README.md` files in sync.
- Cross‑links and metadata
  - Add anchors and links between rules (conditions, senses, actions) and references in classes/spells/monsters.
  - Add “Source pages” metadata to headers to aid verification against the PDF.
- Optional: Split Rules Glossary by term
  - Convert the single glossary file into A–Z folders (one term per file) for deep‑linking to definitions.
- Optional: Docs hygiene
  - Add a CI link checker and simple table/heading lints to catch regressions.
  - Remove aggregate `ALL.md` files once per‑item files are fully validated.
