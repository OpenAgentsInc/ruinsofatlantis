# WebAssembly (WASM) Deployment — Wizard Scene

Audience: engineers preparing the current desktop demo to run in the browser using WebGPU via wgpu + winit. This doc explains what needs to change, which tools to use, and how to bundle and host the app on a website.

Outcome: the “wizard scene” renders in a browser tab with pointer/keyboard input, assets are served over HTTP, and the build is reproducible via a single command.

## At a Glance

- Target: `wasm32-unknown-unknown` using `wgpu` (WebGPU) and `winit 0.30`.
- Bundler: Trunk (recommended) or wasm-bindgen CLI + static index.
- Required code adaptations:
  - Browser logging + panic hook (replace `env_logger`).
  - Async renderer init on web (no `block_on`).
  - Web-safe surface creation (no raw handles on web).
  - Asset I/O over HTTP or embed packs (no `std::fs` at runtime).
  - Deterministic RNG on web (enable `getrandom/js`).
- Assets: pre-decompress any Draco models; ship `assets/` and baked `packs/`.
- Hosting: any static host (GitHub Pages, Netlify, Cloudflare) works.

Current web build defaults (repo specifics)
- Present path: writes linear SceneColor to an offscreen HDR texture, then tonemaps and sRGB‑encodes into a non‑sRGB swapchain (WebGPU UNORM). Direct‑present is disabled on web when the swapchain isn’t sRGB to avoid the classic “all black” output.
- Models: ruins are pre‑decompressed for wasm (`assets/models/ruins.decompressed.gltf`) and embedded; no runtime Draco decode in the browser.
- UI: health bars are on; nameplates are off by default on all targets. You can enable them by setting `RA_NAMEPLATES=1` at runtime (desktop). Browsers don’t have process env; the default is off.
- Ruins placement: distant by default to frame the scene without clutter; set `RA_RUINS_NEAR=1` on desktop to spawn a few near showcase pieces.

## What’s in the Repo Now (relevant parts)

- Entry point uses `env_logger` and synchronous init: `src/main.rs:1`.
- Platform loop uses `pollster::block_on(Renderer::new(...))`: `crates/platform_winit/src/lib.rs:1`.
- Renderer creates a surface using raw window handles (not available on web): `crates/render_wgpu/src/gfx/renderer/init.rs:1`.
- Assets load from disk (GLTF/JSON/pack files) via `std::fs` and path helpers:
  - `crates/render_wgpu/src/gfx/mod.rs:64` (`asset_path`), plus multiple `std::fs` reads in the renderer.
  - `shared/assets/src/skinning.rs:13` and `shared/assets/src/gltf.rs:85` (GLTF import by path).
  - `crates/data_runtime/src/loader.rs:21` and pack readers in the renderer for zone snapshots.

These patterns work on desktop but need web-safe equivalents (HTTP fetch or embedding).

## Tooling Prerequisites

- Rust (stable) with wasm target: `rustup target add wasm32-unknown-unknown`
- Trunk (recommended bundler): `cargo install trunk`
- Optional: `wasm-bindgen-cli` if you prefer manual bundling: `cargo install wasm-bindgen-cli`

## High-Level Plan

1) Add web-specific dependencies and runtime glue.
2) Make platform/renderer initialization web-safe and async.
3) Make asset loading web-safe (HTTP fetch or embed/prepack).
4) Choose a bundling path (Trunk or wasm-bindgen) and create a minimal `index.html`.
5) Build, test locally, and deploy to a static host.

The sections below cover each change in detail and provide exact commands/files to add.

## 1) Web Runtime Glue (logging, panics, RNG)

Problem: `env_logger` and `std::env` don’t work in browsers; panics do not print nicely; RNG needs the web crypto shim.

Do this:

- Add these deps (targeted to wasm):
  - `cargo add --target wasm32-unknown-unknown console_error_panic_hook@0.1.7`
  - `cargo add --target wasm32-unknown-unknown console_log@0.2`
  - `cargo add --target wasm32-unknown-unknown wasm-bindgen@0.2 --features serde-serialize`
  - `cargo add --target wasm32-unknown-unknown wasm-bindgen-futures@0.4`
  - `cargo add --target wasm32-unknown-unknown getrandom@0.2 --features js`

Wire it up in the entrypoint by gating on wasm:

- Replace `env_logger` init with:
  - `console_error_panic_hook::set_once();`
  - `console_log::init_with_level(log::Level::Info).ok();`

Where: `src/main.rs:1`.

Note: keep native logging unchanged behind `#[cfg(not(target_arch = "wasm32"))]` so desktops behave as before.

## 2) Async Initialization on the Web

Problem: `pollster::block_on` cannot block the browser’s main thread. Renderer creation must be awaited without blocking.

Do this:

- Swap to an async init path when compiling for the web (spawn the future and continue once ready). The `winit` event loop on web is driven by the browser; you can build the renderer in `resumed` by spawning a future.

Key change (don’t commit here; shown for context):

- In `crates/platform_winit/src/lib.rs:1`, inside `App::resumed` for wasm:
  - Replace `pollster::block_on(Renderer::new(&window))` with a `wasm_bindgen_futures::spawn_local(async move { ... })` that awaits `Renderer::new(&window)` and then sets `self.state`.
  - Request an initial redraw after the renderer is ready.

This preserves the native path, while avoiding synchronous blocking on web.

## 3) Web-Safe Surface Creation

Problem: raw window/display handles and `create_surface_unsafe` are not available on web.

Do this:

- Use the safe `Instance::create_surface(&window)` path (supported across platforms), instead of constructing a surface from raw handles.

Key change (don’t commit here; shown for context):

- In `crates/render_wgpu/src/gfx/renderer/init.rs:1`:
  - Replace the raw-handle block with:
    - `let instance = wgpu::Instance::default();`
    - `let surface = instance.create_surface(window)?;`
    - Use that `instance`/`surface` for adapter/device requests.

On web, wgpu will target WebGPU; on native, it uses the appropriate backend. Keep `PresentMode::Fifo` (already used) for web stability.

## 4) Asset Loading Strategy (no std::fs on web)

Problem: current loaders use file paths and `std::fs` at runtime. Browsers cannot read from the local filesystem; assets must be fetched over HTTP or embedded.

Choose one of these approaches:

- A) Quickest path — embed small assets and ship packs
  - Bake packs: `cargo xtask build-packs` (produces `packs/`), commit what you need for the demo (zone snapshot, spells pack).
  - Embed small JSON/config (if needed) via `include_bytes!` or compile-time embedding (requires code changes), and adjust loaders to read from memory on web.
  - Copy large assets (GLTF/PNG) into the final site and fetch them via relative URLs.

- B) HTTP for everything — fetch GLTF/images and packs
  - Replace `gltf::import(path)` with `gltf::Gltf::from_slice(...)` + `gltf::import_slice(...)` using fetched bytes.
  - Replace `std::fs::read*` pack reads with HTTP fetch (`wasm-bindgen`/`web-sys`/`gloo-net`).
  - Keep relative URLs stable so they can be served from `assets/` and `packs/` directly.

Either way, ensure no asset reads occur via `std::fs` on web. Places to update:

- `crates/render_wgpu/src/gfx/mod.rs:64` (`asset_path` helper) — return HTTP-relative paths on web.
- GLTF load sites (renderer + `shared/assets`):
  - `shared/assets/src/skinning.rs:13`
  - `shared/assets/src/gltf.rs:85`
- Any direct `std::fs::read` in the renderer and terrain loaders.

Draco note: we do not decode Draco at runtime. Ensure any Draco-compressed GLTFs are pre-decompressed once:

```
cargo run -p gltf-decompress -- assets/models/ruins.gltf assets/models/ruins.decompressed.gltf
```

Repo specifics
- On wasm we embed and import `assets/models/ruins.decompressed.gltf` directly (no HTTP fetch, no Draco), and keep `rock.glb` embedded as well. If you add new models for the web build that were Draco‑compressed, run the command above and embed the `.decompressed.gltf` instead of the original.

## 5) DOM Canvas Wiring (Web)

`winit 0.30` can target web. Two small items help stability:

- Ensure the window’s canvas is attached to the DOM (if not already). When needed, use `winit::platform::web::WindowExtWebSys` to access the canvas and append it to `document.body`.
- Consider an explicit canvas size or CSS to fill the viewport and set `touch-action: none;` to prevent browser gestures from interfering with input.

## 6) Bundling with Trunk (recommended)

Trunk automates `wasm-bindgen`, serves assets, and provides `trunk serve` for local dev.

Add a minimal `index.html` (example):

```html
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Ruins of Atlantis — Wizard Scene</title>
    <!-- Copy runtime assets into dist/ at build time -->
    <link data-trunk rel="copy-dir" href="assets" />
    <link data-trunk rel="copy-dir" href="packs" />
    <style>
      html, body { margin: 0; height: 100%; background: #000; }
      canvas { display: block; width: 100vw; height: 100vh; touch-action: none; }
    </style>
  </head>
  <body>
    <canvas id="app-canvas"></canvas>
    <script>
      // Feature check: WebGPU support
      if (!('gpu' in navigator)) {
        document.body.innerHTML = '<p style="color:#fff;font-family:monospace;padding:1rem">This browser does not support WebGPU. Use Chrome 113+, Edge 113+, or Safari TP with WebGPU enabled.</p>';
      }
    </script>
  </body>
  </html>
```

Optional `Trunk.toml` (example):

```toml
[build]
target = "wasm32-unknown-unknown"
release = true

[watch]
ignore = ["target", "packs"]
```

Build locally:

```
rustup target add wasm32-unknown-unknown
cargo install trunk
trunk serve --open
# or
trunk build --release
```

The output lives in `dist/`. You can host the `dist/` directory on any static host.

Selecting a specific Zone in the browser
- Append `?zone=<slug>` to the URL to bypass the Zone Picker.
- Examples:
  - Local dev (Trunk): `http://127.0.0.1:8080/?zone=cc_demo`
  - Live site route: `/play?zone=cc_demo`

Base path note: when deploying under a subpath (e.g., GitHub Pages `/ruinsofatlantis/`), build with `trunk build --release --public-url /ruinsofatlantis/` so asset URLs resolve.

### Integrating into existing sites (Laravel / React / Inertia / Vite)

You don’t need to adopt Trunk for your whole site — you can treat the wasm app as a static bundle and include it.

Option A — Use Trunk to produce `dist/` and copy artifacts:
- `trunk build --release [--public-url /subpath/]`
- Copy `dist/*` into your app’s public folder (e.g., `public/wasm/`). This will include:
  - `index.html` (if you used our template)
  - One or more fingerprinted JS files (e.g., `ruinsofatlantis-<hash>.js`)
  - A `.wasm` file (e.g., `ruinsofatlantis-<hash>_bg.wasm`)
  - `assets/` and `packs/` (copied via `<link data-trunk rel="copy-dir" ...>`)

To embed on a page you control (Blade/React component):
- Add a container `<div id="wasm-app"></div>` or a full page route.
- Add a module script that dynamically imports the generated JS and initializes the module. Example Blade snippet:

```blade
@php($base = asset('wasm'))
<link rel="preload" href="{{ $base }}/ruinsofatlantis-xxxx.js" as="script" crossorigin>
<script type="module">
  import init from '{{ $base }}/ruinsofatlantis-xxxx.js';
  init(); // the module attaches to the canvas created by winit
</script>
```

Notes
- The filenames are fingerprinted; update the `xxxx` when you redeploy.
- Keep the `.wasm`, `.js`, `assets/`, and `packs/` in the same relative folder so runtime fetches resolve.
- If your app sits under `/app`, ensure paths line up; with Trunk, use `--public-url /app/wasm/`.

Option B — Manual wasm‑bindgen (Vite/React)
- Build wasm: `cargo build --release --target wasm32-unknown-unknown`
- Run wasm‑bindgen for web output into `public/wasm/`.
- In React, import the generated JS in an effect:

```ts
useEffect(() => {
  (async () => {
    const init = (await import('/wasm/ruinsofatlantis.js')).default;
    await init();
  })();
}, []);
```

Ensure `/wasm/ruinsofatlantis_bg.wasm`, `/wasm/assets/*`, and `/wasm/packs/*` are served statically.

Caching tips
- The hashed filenames from Trunk are cache‑friendly. Set long `Cache-Control` on `.js`/`.wasm` and version by redeploying.
- Keep `assets/` and `packs/` cacheable; bump content to invalidate.

## 7) Alternative: Manual wasm-bindgen Flow

If you prefer to avoid Trunk:

```
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --target web --no-typescript \
  --out-dir web \
  target/wasm32-unknown-unknown/release/ruinsofatlantis.wasm
```

Create a simple `web/index.html` that loads the generated `ruinsofatlantis.js`, and copy `assets/` and `packs/` next to it. Serve the `web/` directory with any static file server.

## 8) Hosting Options

- GitHub Pages: publish `dist/` on the `gh-pages` branch. Build with `--public-url /<repo>/` and use relative asset links.
- Netlify/Cloudflare Pages: set build command to `trunk build --release` and publish directory to `dist/`.
- Any static HTTP host: ensure `assets/` and `packs/` are reachable at the same relative paths the app expects.

## 9) Browser Support & Fallbacks

- WebGPU: Chrome 113+, Edge 113+, recent Safari (may require enabling the WebGPU flag). No special headers are needed unless using shared memory.
- WebGL fallback (optional): if you need broader support, enable `wgpu`’s WebGL backend feature and avoid formats not supported by WebGL (e.g., `Rgba16Float`). This requires code changes to choose formats conditionally.

## 10) Known Gaps to Close (code changes required)

This repo compiles and runs natively today. To run in browsers, make these concrete adjustments:

- Entry logging and panic hook
  - `src/main.rs:1`: gate `env_logger` on native; use `console_log` + `console_error_panic_hook` on web.

- Async renderer init
  - `crates/platform_winit/src/lib.rs:1`: replace `pollster::block_on` with `wasm_bindgen_futures::spawn_local(async move { let r = Renderer::new(&window).await; ... })` under `#[cfg(target_arch = "wasm32")]`.

- Surface creation
  - `crates/render_wgpu/src/gfx/renderer/init.rs:1`: use `wgpu::Instance::default()` and `instance.create_surface(window)?` across all platforms. Remove reliance on `SurfaceTargetUnsafe` on web.

- Asset I/O replacement
  - Replace `std::fs` reads in the renderer and loaders with either HTTP fetch (using `wasm-bindgen`/`web-sys`/`gloo-net`) or compile-time embedding for small data. Key sites:
    - `crates/render_wgpu/src/gfx/mod.rs:64` (pathing)
    - `shared/assets/src/skinning.rs:13` and `shared/assets/src/gltf.rs:85` (GLTF import)
    - Terrain snapshot/pack reads (`crates/render_wgpu/src/gfx/terrain.rs`, renderer pack reads around colliders)
    - Spell/spec reads (`crates/data_runtime/src/loader.rs:21`) — prefer consuming baked packs in web builds

- RNG support on web
  - Ensure `getrandom/js` is enabled so `rand` seeds correctly under wasm.

- Present/gamma correctness on web (repo fixed)
  - Non‑sRGB swapchains are common in browsers (`BGRA8Unorm`). We avoid direct‑present in that case and sRGB‑encode in the present pass to prevent a dark/black image. If you rework the render path, keep this invariant or ensure the swapchain is sRGB.

- Pointer lock and input niceties
  - For right-drag orbit input, on web you may need to explicitly request pointer lock (via `winit` web extension traits) on mouse-down and exit on mouse-up.

## 11) Assets & Packs Checklist

- Decompress any Draco GLTFs once (see above) and commit the decompressed versions used by the demo.
- Bake and commit packs used by the demo scene:
  - `cargo xtask build-packs`
  - For the demo zone, ensure `packs/zones/wizard_woods/snapshot.v1/*` exists.
- When bundling, ensure `assets/` and `packs/` are copied to your site output:
  - With Trunk: `<link data-trunk rel="copy-dir" href="assets" />` and `<link data-trunk rel="copy-dir" href="packs" />` in `index.html`.

## 12) Build & Verify

Local dev (Trunk):

```
rustup target add wasm32-unknown-unknown
cargo install trunk
trunk serve --open
```

Production bundle:

```
trunk build --release
```

Smoke test:

- Load the page; verify sky/terrain render, wizard ring, input camera orbit, and basic HUD draw. Watch the console for shader or device errors.

## 13) Troubleshooting

- Blank page, console shows `WebGPU not supported`:
  - Use a WebGPU-capable browser, or enable the feature flag in Safari.

- `getCurrentTexture` or `SurfaceError::Outdated` loops:
  - Occurs if the canvas size is 0×0 or not attached to the DOM. Ensure the canvas is in the document and sized via CSS.

- Panic on wasm due to `std::env` or `std::fs`:
  - Confirm logging/panic hooks are active for web and that all runtime file reads are removed or replaced by HTTP/embedding.
  - Environment variables in browsers are not available. Our UI toggles default to safe values on web (e.g., nameplates off). If you need runtime toggles in browsers, add URL query params or JS configuration rather than `std::env`.

- No NPC movement / randomness appears frozen:
  - Ensure `getrandom/js` is enabled so `rand` works on wasm.

---

Appendix: Why Trunk?

- It wraps the wasm-bindgen step, copies assets, live-reloads, and is the most friction-free path for a winit + wgpu app targeting the web.

Security note: do not embed secrets; use relative HTTP assets; static hosting is sufficient.

---

Appendix B: One‑shot Deploy to the Laravel Site

For a local sibling site repo (defaulting to `$SITE_REPO` or `$HOME/code/ruinsofatlantis.com`), use the helper script to build, copy artifacts into `public/`, update the Blade view (`resources/views/play.blade.php`), commit, push, and open a PR:

```
# From the app repo root
RUN_CI=1 scripts/deploy_wasm_to_site.sh

# Optional environment vars
# SITE_REPO=/path/to/ruinsofatlantis.com
# PUBLIC_SUBDIR=wasm   # copy hashed JS/WASM under public/wasm instead of public/
# NO_PR=1              # skip PR creation
```

What it does
- Ensures the wasm toolchain and `trunk` exist.
- Runs `cargo xtask ci` if `RUN_CI=1`.
- Builds with `trunk build --release`.
- Copies `dist/assets/` and `dist/packs/` to the site’s `public/` (rsync with delete).
- Publishes the hashed JS and WASM at the public root (or `public/wasm` if `PUBLIC_SUBDIR` is set).
- Edits `resources/views/play.blade.php` to point to the new hashed filenames.
- Creates a branch, commits, pushes, and opens a PR (requires `gh` CLI).

## 14) Versioned GitHub Releases (immutable WASM bundles)

Goal: produce immutable, versioned WASM artifacts zipped from `dist/` and attached to a GitHub Release for a specific tag (or on-demand).

Workflow: `.github/workflows/wasm-release.yml`

- Triggers:
  - Tag push: `v*` → builds from the tag and attaches `ruinsofatlantis-wasm-<tag>.zip` (+ `.sha256`) to the Release.
  - Manual: `workflow_dispatch` with inputs:
    - `tag` (optional): build that tag. If omitted, builds current commit and opens a draft Release with a synthetic tag `draft-<sha7>`.
    - `public_url` (optional): passes `--public-url` to Trunk (e.g., `/ruinsofatlantis/` or `/wasm/`).

What it builds
- Runs `cargo xtask ci` and `cargo xtask build-packs`.
- Builds via `trunk build --release` (including `assets/` and `packs/` copied via `index.html`).
- Packages `dist/` into `ruinsofatlantis-wasm-<tag>.zip` and generates a `.sha256` checksum.
- Attaches both files to the Release with `softprops/action-gh-release`.

Manual fallback (local)
```
trunk build --release
zip -r ruinsofatlantis-wasm-<tag>.zip dist/
gh release create <tag> ruinsofatlantis-wasm-<tag>.zip \
  --title "WASM <tag>" --notes "Trunk build from <sha>"
```

Notes
- Keep the `.zip` as the canonical immutable artifact; filenames inside are content‑hashed by Trunk.
- If hosting under a subpath, consider passing `--public-url` so runtime asset URLs resolve.

Run a release bundle locally
- Unzip the Release asset (`ruinsofatlantis-wasm-<tag>.zip`) and serve the unzipped folder with any static HTTP server (browsers block `file://` for WASM):

  - Python (3.x): `python3 -m http.server 8080 --directory /path/to/unzipped`
  - Node (npx): `npx --yes serve -p 8080 /path/to/unzipped`
  - Rust (miniserve): `cargo install miniserve && miniserve /path/to/unzipped -p 8080`

Then open http://127.0.0.1:8080 in your browser. Ensure you serve the directory that contains `index.html`, the hashed `ruinsofatlantis-*.js`, `*_bg.wasm`, and `assets/` + `packs/`.
