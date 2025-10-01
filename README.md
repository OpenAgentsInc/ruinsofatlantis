# Ruins of Atlantis

An MMORPG built from scratch in Rust

- [Game Design Document](GDD.md)

<img width="2544" height="1379" alt="wiz1" src="https://github.com/user-attachments/assets/e5f96cc1-d244-4909-a720-94acce00338a" />

## Play The Alpha / Build From Source (Public Game Repo)

The weekly alpha builds are playable on the web, and you can also build the game from source locally if your machine supports WebGPU.

- Play in your browser (recommended): https://ruinsofatlantis.com/play
- Build from source (developers/power users): follow the steps below

### Prerequisites
- Rust toolchain via `rustup` (stable). If needed: `curl https://sh.rustup.rs -sSf | sh`
- Git LFS for large binary assets: `git lfs install`
- A WebGPU‑capable GPU + up‑to‑date browser (Chrome works well on macOS today)

### Clone with assets
```
git clone https://github.com/OpenAgentsInc/ruinsofatlantis
cd ruinsofatlantis
git lfs pull
```

### Native (desktop) build
```
# optional: run formatting, lint, shader checks, and tests
cargo xtask ci

# run the app
cargo run

# with logs
RUST_LOG=info cargo run
```

### Web/WASM (local dev)
```
rustup target add wasm32-unknown-unknown
cargo install trunk

# serve the WASM build locally (http://127.0.0.1:8080)
trunk serve --release
```

Notes
- If a GLTF model uses Draco compression, decompress it once before runtime:
  - `cargo run -p gltf-decompress -- assets/models/ruins.gltf assets/models/ruins.decompressed.gltf`
- A simple standalone viewer exists:
  - `cargo run -p model-viewer -- assets/models/wizard.gltf`
- Current keybinds: `P` (perf), `O` (orbit), `H` (HUD), `Space`/`[`/`]` and `-`/`=` control time‑of‑day.
