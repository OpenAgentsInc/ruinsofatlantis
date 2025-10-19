Short answer: **yes—use Bevy as a toolkit, not “the engine.”**
Adopt the parts that buy us time (ECS, reflection, fixed-timestep, relationships, asset/gltf for tools), and keep our bespoke bits (renderer, platform/winit, deterministic server/netcode). That gives us Bevy’s mature core without getting boxed into its full stack.

## Where Bevy helps us right now

* **ECS as our core data model (bevy_ecs 0.17).** Standalone, fast, massively parallel, and easy to embed in non-Bevy apps. We can run our own schedules/ticks and never pull in Bevy’s window/render stack. ([[Docs.rs](https://docs.rs/crate/bevy_ecs/latest?utm_source=chatgpt.com)][1])
* **Fixed-timestep simulation and time utilities.** Bevy’s `FixedUpdate` schedule + time APIs are exactly what we need for server-authoritative, deterministic loops (and we can interpolate visuals on the client). ([[Docs.rs](https://docs.rs/bevy/latest/bevy/time/struct.Fixed.html?utm_source=chatgpt.com)][2])
* **Entity relationships (0.16+) for hierarchy and ownership graphs.** Useful for riders↔mounts, attachments, interest groups, etc. It’s now first-class in ECS. ([[Bevy](https://bevy.org/learn/migration-guides/0-15-to-0-16/?utm_source=chatgpt.com)][3])
* **Reflection & data-driven content (bevy_reflect).** Runtime type info + (de)serialization for our Worldsmithing/Wishcrafting content, save files, and debug inspectors. (Mind the limitations around references.) ([[Docs.rs](https://docs.rs/bevy_reflect/latest/bevy_reflect/?utm_source=chatgpt.com)][4])
* **Asset & glTF in tooling (not in the runtime).** Use `bevy_asset`/`bevy_gltf` in offline tools (model viewer, baker, animation merger) to parse scenes and generate our packed formats; avoid shipping the full asset server in the client. ([[Docs.rs](https://docs.rs/bevy/latest/bevy/gltf/index.html?utm_source=chatgpt.com)][5])

## Where we should **not** lean on Bevy (for an MMO like ours)

* **Rendering:** keep our custom WGPU renderer (impostors, crowd tech, custom pipelines). Bevy’s renderer is good and keeps improving (GPU-driven, decals, atmospheric scattering; even experimental ray-tracing in 0.17), but its render-graph and materials would slow our bespoke path. ([[Bevy](https://bevy.org/news/bevys-fifth-birthday/?utm_source=chatgpt.com)][6])
* **Networking:** Bevy has no built-in MMO stack. Community crates exist—`bevy_replicon` (server-auth replication), `bevy_quinnet` (QUIC), `bevy_renet`, `bevy_ggrs` (rollback)—great for prototypes, but we should keep our own `net_core` (QUIC + snapshots, anti-cheat, interest mgmt). ([[Docs.rs](https://docs.rs/bevy_replicon/latest/bevy_replicon/?utm_source=chatgpt.com)][7])
* **glTF Draco at runtime:** treat Draco as **offline**. Bevy’s glTF loader is solid, but Draco decoding isn’t first-party; keep our current preprocess (meshopt/ktx2 or pre-decompress). ([[GitHub](https://github.com/bevyengine/bevy/issues/11350?utm_source=chatgpt.com)][8])
* **Whole-engine ownership:** avoid pulling in `DefaultPlugins` / `bevy_winit` / `bevy_render` for the shipping client; we already have those layers.

## Two viable architectures

### Option A — “Bevy-in-the-middle” (recommended)

* **Server:** headless crate using `bevy_ecs` + `bevy_time`; our loop manually runs a `Sim` schedule at fixed Hz; `net_core` handles I/O.
* **Client:** our WGPU renderer; a thin bridge mirrors ECS state into draw lists; use `bevy_transform_interpolation`-style visual smoothing.
* **Tools:** a separate Bevy app (with `bevy_gltf`, `bevy_asset`) for model viewing, animation graphs, packfile baking.
  Pros: maximum control, minimal coupling, shared ECS types.
  Cons: we write some glue.

### Option B — “Full Bevy client, custom server”

* **Server:** same as above.
* **Client:** a Bevy app (winit + renderer) for speed to first-playable; later migrate rendering to our pipelines.
  Pros: fastest “fun” path, editor/inspector ecosystem.
  Cons: migration off Bevy’s renderer later can be costly.

## Glue we’ll add (lightweight)

* **`roa_ecs`**: a crate exporting our Components/Events/Schedules (pure `bevy_ecs`).
* **`roa_sim`**: fixed-tick executor (e.g., 30/60 Hz) that calls `world.run_schedule(Sim)`.
* **`roa_bridge_render`**: reads ECS transforms/instances into our renderer’s scene graph + keeps a small interpolation buffer.
* **`roa_tools/gltf-baker`**: Bevy app that loads glTF, resolves skins/animations, writes our compact runtime assets.

## Minimal headless loop (server or offline sim)

```rust
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use std::time::{Duration, Instant};

#[derive(Component)] struct Position(glam::Vec3);
#[derive(Component)] struct Velocity(glam::Vec3);

fn integrate(mut q: Query<(&mut Position, &Velocity)>) {
    for (mut p, v) in &mut q {
        p.0 += v.0 * (1.0 / 60.0);
    }
}

fn main() {
    let mut world = World::new();
    world.spawn((Position(glam::Vec3::ZERO), Velocity(glam::Vec3::X)));

    let mut sim = Schedule::default();
    sim.add_systems(integrate);

    let tick = Duration::from_millis(16); // 60 Hz
    let mut next = Instant::now();
    loop {
        while Instant::now() >= next {
            sim.run(&mut world); // deterministic tick
            next += tick;
        }
        // …net I/O + snapshotting here…
    }
}
```

(You can do this without `bevy_app`; schedules run fine standalone.) ([[Reddit](https://www.reddit.com/r/bevy/comments/1mewr7b/how_do_i_use_events_with_only_bevy_ecs/?utm_source=chatgpt.com)][9])

## Risks & trade-offs

* **API churn:** Bevy is moving fast (current: **0.17**, released Sep 30, 2025). Pin versions per-crate and plan migrations each minor (they provide guides). ([[Bevy](https://bevy.org/news/bevy-0-17/?utm_source=chatgpt.com)][10])
* **Compile time:** feature-gate aggressively; prefer `bevy_ecs`/`bevy_time` directly in runtime crates.
* **Determinism:** Bevy doesn’t give it to you; we enforce it in our systems (fixed tick, avoid platform-divergent math).

## Concrete “do this next”

1. **Adopt ECS everywhere.** Create `crates/roa_ecs` exporting components/resources/events shared by server & client.
2. **Stand up the fixed-tick server loop** (as above) and plug in our snapshot/QUIC pipeline.
3. **Fork the model viewer into a Bevy tool**: `roa_tools/model_viewer` using `bevy_gltf` + the new animation graph APIs; export to our packfiles. ([[Docs.rs](https://docs.rs/bevy/latest/bevy/animation/index.html?utm_source=chatgpt.com)][11])
4. **Client bridge:** write a `TransformSync` that mirrors ECS transforms into our render scene and adds an interpolation buffer (inspired by `bevy_transform_interpolation`). ([[GitHub](https://github.com/Jondolf/bevy_transform_interpolation?utm_source=chatgpt.com)][12])
5. **Only if we want quick net prototypes**: evaluate `bevy_replicon` + `bevy_quinnet` in a sandbox branch; keep production on `net_core`. ([[Docs.rs](https://docs.rs/bevy_replicon/latest/bevy_replicon/?utm_source=chatgpt.com)][7])

If we follow this, we get Bevy’s **best parts** (ECS, time, relationships, reflection, glTF for tools) while keeping the custom pieces that define Ruins of Atlantis.

*(References: Bevy 0.17 release & migration notes; ECS/Time/Relationships docs; bevy_reflect; glTF & toolchain; community networking crates.)* ([[Bevy](https://bevy.org/news/bevy-0-17/?utm_source=chatgpt.com)][10])

[1]: https://docs.rs/crate/bevy_ecs/latest?utm_source=chatgpt.com "bevy_ecs 0.17.0"
[2]: https://docs.rs/bevy/latest/bevy/time/struct.Fixed.html?utm_source=chatgpt.com "Fixed in bevy::time - Rust"
[3]: https://bevy.org/learn/migration-guides/0-15-to-0-16/?utm_source=chatgpt.com "Migration Guide: 0.15 to 0.16"
[4]: https://docs.rs/bevy_reflect/latest/bevy_reflect/?utm_source=chatgpt.com "bevy_reflect - Rust"
[5]: https://docs.rs/bevy/latest/bevy/gltf/index.html?utm_source=chatgpt.com "bevy::gltf - Rust"
[6]: https://bevy.org/news/bevys-fifth-birthday/?utm_source=chatgpt.com "Bevy's Fifth Birthday"
[7]: https://docs.rs/bevy_replicon/latest/bevy_replicon/?utm_source=chatgpt.com "bevy_replicon - Rust"
[8]: https://github.com/bevyengine/bevy/issues/11350?utm_source=chatgpt.com "Robust GLTF Extension Support - bevyengine/bevy"
[9]: https://www.reddit.com/r/bevy/comments/1mewr7b/how_do_i_use_events_with_only_bevy_ecs/?utm_source=chatgpt.com "How do I use events with only bevy_ecs? - bevy"
[10]: https://bevy.org/news/bevy-0-17/?utm_source=chatgpt.com "Bevy 0.17"
[11]: https://docs.rs/bevy/latest/bevy/animation/index.html?utm_source=chatgpt.com "bevy::animation - Rust"
[12]: https://github.com/Jondolf/bevy_transform_interpolation?utm_source=chatgpt.com "Jondolf/bevy_transform_interpolation"
