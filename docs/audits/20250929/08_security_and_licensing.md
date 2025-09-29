# Security & Licensing

Security
- No secrets in repo; ensure `.env` patterns and `.gitignore` remain intact; provide `.env.example` for local configs.
- Prefer explicit config files under `config/` with sample defaults; document required env vars in `README.md`.
- Consider `cargo audit` in CI (or via `cargo deny advisories`) to catch vulnerable deps.

Licensing & SRD
- `LICENSE` (Apache-2.0) and `NOTICE` exist; `GDD.md` maintains SRD usage.
- Ensure all SRD 5.2.1 content attribution remains exact in `NOTICE`; avoid trademarked terminology.
- Track third-party license notices (e.g., Draco decoder, GLTF crates) in `NOTICE`.

Assets & LFS
- Ensure binary assets are tracked via git‑lfs (models, textures). Add a check in CI to warn on large non-LFS files under `assets/`.
- Document asset ingestion policy (preferred formats, decompression requirements, and naming) in `docs/systems`.

