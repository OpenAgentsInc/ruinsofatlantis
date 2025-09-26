//! ra-assets: Shared asset-loading library crate (v1 wrapper).
//!
//! For v1, this crate wraps the existing ruinsofatlantis `assets` module so
//! tools (like the standalone model viewer) can load models without depending
//! on the whole engine. Longer-term we can migrate/own the ingest architecture
//! here behind format-agnostic APIs.

pub mod util;

// Re-export useful CPU-side types and loader entry points for v1.
pub use ruinsofatlantis::assets::load_gltf_skinned;
pub use ruinsofatlantis::assets::types::{
    AnimClip, SkinnedMeshCPU, TextureCPU, TrackQuat, TrackVec3, VertexSkinCPU,
};

