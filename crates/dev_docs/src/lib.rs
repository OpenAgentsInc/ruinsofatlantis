//! Ruins of Atlantis — Developer Documentation (rustdoc aggregator)
//!
//! This crate aggregates important Markdown docs into a rustdoc site so you can browse
//! design and systems documentation via `cargo doc -p dev_docs --no-deps`.
//!
//! Tips
//! - Build: `cargo doc -p dev_docs --no-deps` (add `--open` to open in a browser)
//! - No doctests are run for included Markdown (see doctest=false in Cargo.toml)
//! - This is a short‑term path; we can add mdBook or a site later.

pub mod index {
    #![doc = include_str!("../../../docs/README.md")]
}

pub mod gdd {
    #![doc = include_str!("../../../docs/gdd/README.md")]

    pub mod mechanics_overview {
        #![doc = include_str!("../../../docs/gdd/02-mechanics/overview.md")]
    }
    pub mod destructibility {
        #![doc = include_str!("../../../docs/gdd/02-mechanics/destructibility.md")]
    }
}

pub mod systems {
    pub mod zones {
        #![doc = include_str!("../../../docs/systems/zones.md")]
    }
    pub mod telemetry {
        #![doc = include_str!("../../../docs/systems/telemetry.md")]
    }
    pub mod frame_graph {
        #![doc = include_str!("../../../docs/systems/frame_graph.md")]
    }
    pub mod model_loading {
        #![doc = include_str!("../../../docs/systems/model_loading.md")]
    }
    pub mod sky_weather {
        #![doc = include_str!("../../../docs/systems/sky_weather.md")]
    }
    pub mod terrain_biomes {
        #![doc = include_str!("../../../docs/systems/terrain_biomes.md")]
    }
    pub mod controls {
        #![doc = include_str!("../../../docs/systems/controls.md")]
    }
    pub mod voxel_destruction_status {
        #![doc = include_str!("../../../docs/systems/voxel_destruction_status.md")]
    }
    pub mod spells_mvp {
        #![doc = include_str!("../../../docs/systems/spell_ability_system.md")]
    }
}

pub mod architecture {
    pub mod ecs_guide {
        #![doc = include_str!("../../../docs/architecture/ECS_ARCHITECTURE_GUIDE.md")]
    }
}

