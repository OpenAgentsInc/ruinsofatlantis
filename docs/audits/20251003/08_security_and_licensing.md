# Security & Licensing Audit

Security
- No secrets in repo; keep `.env.example`; avoid PII in logs/snapshots.
- Networking to add: authenticate channels; sanitize client inputs; server authority.

Licensing/SRD
- SRD 5.2.1 attribution maintained in NOTICE; keep GDD SRD section updated on usage.
- 3rd‑party notices: ensure vendor/ decoders are attributed; consider `cargo about` for report.
