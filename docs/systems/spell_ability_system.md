# Spell & Ability System (MVP)

Author JSON specs (`/data/spells/*.json`) validated by `data_runtime` models.

- Build pipeline (future): JSON → `packs/spellpack.v1.bin` with stable IDs/hashes.
- Sim loads the pack; client/server assert identical pack hashes.

