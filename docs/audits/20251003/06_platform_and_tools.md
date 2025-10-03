# Platform & Tools Audit

Platform (`platform_winit`)
- Input mapping is code‑based; introduce `client_core` Input → Command system with rebinding.
- Window/event loop is clean; ensure all gameplay toggles move out of renderer.

Tools/xtask
- Keep `xtask ci` as golden path; add optional `cargo deny` and doc build.
- Asset tools live under `tools/`; keep runtime strictly side‑effect free.
