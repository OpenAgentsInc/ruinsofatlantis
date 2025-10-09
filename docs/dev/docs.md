Here are the practical options to automatically generate Rust-based documentation from our repo, plus what’s already in place and what we’d need.

  Current state

  - Cargo/rustdoc: Works out of the box for API docs via rustdoc comments (///, //!). We don’t have a dedicated “docs build” task or CI publishing for docs.
  - Docs content: Lives as Markdown under docs/** and GDD split under docs/gdd/**. Not currently integrated into rustdoc output.
  - xtask: No docs subcommand; no mdBook config present.

  Options

  1. Cargo doc (rustdoc) for API docs

  - What it does: Generates API docs from /// and //! comments across all crates.
  - How to run: cargo doc --workspace --all-features --no-deps
  - Pros:
      - Zero extra deps; integrates with doctests; supports intra-doc links.
      - Can include Markdown files directly into docs: #![doc = include_str!("../README.md")] or #[doc = include_str!("../../docs/systems/zones.md")].
  - Cons:
      - Best for API and code-adjacent docs; not ideal for large design books.
  - What we’d add:
      - Add crate-level docs: #![doc = include_str!("../README.md")] per crate where useful.
      - Sprinkle #[doc = include_str!(...)] modules to surface key docs from docs/systems/** (e.g., frame_graph, zones).
      - Optional: enforce docs on APIs with #![deny(missing_docs)] in crates ready for it.
      - Optional: add an xtask target: cargo xtask docs → runs cargo doc with flags and writes to dist/docs.

  2. “Docs aggregator” crate (Rustdoc site from Markdown files)

  - What it is: A small workspace crate (e.g., dev_docs) whose sole purpose is to publish a rustdoc site composed of our Markdown files.
  - How it works: lib.rs contains modules like:
      - #![doc = include_str!("../../docs/README.md")]
      - pub mod zones { #![doc = include_str!("../../docs/systems/zones.md")] }
      - Repeat for other docs you want in the rustdoc site.
  - Pros:
      - Keeps one docs build tool (rustdoc).
      - Allows rust fenced code blocks to run as doctests.
  - Cons:
      - Manual mapping of files → modules (a one-time setup).
  - What we’d add:
      - Create crates/dev_docs (publish = false) and wire includes for top docs.
      - Add cargo doc -p dev_docs --no-deps to xtask and/or CI.
      - Optional: static hosting (see “Hosting” below).

  3. mdBook for design/guide docs

  - What it does: Builds a static “book” from Markdown (great for guides like GDD, systems).
  - How to run: mdbook build docs (requires mdbook (cargo install mdbook)), with a book.toml and docs/book/SUMMARY.md.
  - Pros:
      - Native navigation, search, and theming; ideal for non-API documentation.
  - Cons:
      - A second documentation tool (in addition to rustdoc).
  - What we’d add:
      - Add book.toml and docs/book/SUMMARY.md mapping our existing docs.
      - Add an xtask docs-book command and a CI job to build/publish.

  4. docs.rs (hosted API docs)

  - What it does: Builds and hosts crate API docs on docs.rs.
  - Pros:
      - Zero infra for hosting.
  - Cons:
      - Requires publishing crates to crates.io; not ideal for private/in-progress workspace crates.
  - Use if/when we publish crates publicly.

  5. rustdoc JSON for custom pipelines

  - What it is: Generate JSON output from rustdoc for custom site/search tooling (nightly: RUSTDOCFLAGS="--output-format json" cargo +nightly doc -Z unstable-options).
  - Pros:
      - Machine-readable; can power custom UIs.
  - Cons:
      - Nightly-only; extra infra to render.
  - Only necessary if we want custom doc tooling.

  Hosting options

  - Self-host rustdoc output: cargo doc --workspace --all-features --no-deps -Z unstable-options --output-dir dist/docs then push to GH Pages or our site repo.
  - mdBook: mdbook build docs/book to dist/book; host same as above.

  Recommended plan

  - Short term (low friction, Rust-native):
      - Add a “docs aggregator” crate (dev_docs) to include key docs from docs/systems/** and top-level README.
      - Add an xtask command xtask docs to run cargo doc -p dev_docs --no-deps.
      - Optional: add #![doc = include_str!("../README.md")] to crates with useful top-level context; start adding /// to public APIs we rely on (renderer, ECS
  components, data_runtime loaders).
  - Medium term (nicer site UX):
      - Add mdBook for the design/book side (GDD + systems); keep rustdoc for API.
      - Add CI to build and publish both books (API rustdoc + mdBook) on push to main.

  What’s needed to enable each

  - Cargo doc (now): Nothing—works today. To surface non-API docs, add #[doc = include_str!("...md")] in relevant crates.
  - Aggregator crate: Add a new workspace crate; wire include_str! paths; add xtask + optional publishing script.
  - mdBook: Install mdbook, add minimal book.toml + SUMMARY.md, map our existing docs; add CI/build step.

If you want, I can:

  - Scaffold the dev_docs crate and wire in our top design/system docs.
  - Add xtask docs to build docs locally and a script to publish to GitHub Pages or the sibling site.
  - Draft a minimal mdBook config mapping the current docs/ tree so we can compare against a rustdoc-only approach.

## Status Update (Implemented) and Next Steps

What’s implemented now
- Added a short‑term Rustdoc aggregator crate: `crates/dev_docs`.
  - Aggregates key Markdown docs via `include_str!` into a browsable Rustdoc site.
  - Wired into the workspace. Build locally with: `cargo doc -p dev_docs --no-deps --open`.
  - Included docs: `docs/README.md`, `docs/gdd/README.md`, GDD Mechanics (overview, destructibility), and `docs/systems/**` (zones, telemetry, frame graph, model loading, sky/weather, terrain/biomes, controls, voxel destruction status, spells MVP), plus `docs/architecture/ECS_ARCHITECTURE_GUIDE.md`.
- Cleaned up docs structure and links:
  - Canonical Zones spec: `docs/systems/zones.md` (removed older duplicates).
  - Telemetry moved to `docs/systems/telemetry.md` (removed `observability/` references).
  - GDD split linked and up to date; added “Destructibility” mechanics page.
- Agent guidance updated (`AGENTS.md`) to use `docs/gdd/README.md` and keep work on `main` by default.

What’s next (short‑term)
- Add `xtask docs` to build `dev_docs` and optionally copy output to `dist/docs/` for publishing.
- Expand `dev_docs` coverage incrementally (more `docs/gdd/**` sections and any additional systems pages).
- Tag non‑Rust code blocks in Markdown with `text` or `ignore` to avoid rustdoc warnings.

What’s next (medium‑term)
- Add mdBook for design/navigation (GDD + systems) while keeping Rustdoc for API.
  - Create `book.toml` + `docs/book/SUMMARY.md`; map current docs.
  - Add CI job to build and publish both (Rustdoc API + mdBook) to GitHub Pages or our site.
- Optionally add crate‑level `#![doc = include_str!("../README.md")]` for select crates and strengthen API docs with `///`.

Stretch goals
- Add search and cross‑linking conventions (heading anchors, consistent section headers).
- Consider `rustdoc --output-format json` (nightly) only if we decide to build a custom docs UI later.
