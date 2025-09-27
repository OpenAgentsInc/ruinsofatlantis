//! ra-assets: Shared asset-loading library crate.
//!
//! This crate owns the CPU-side asset ingest (GLTF/GLB v1) and exposes
//! simple types suitable for GPU upload. It is renderer-agnostic.

pub mod draco;
/// Experimental/feature-gated loaders and helpers.
pub mod fbx;
pub mod gltf;
pub mod obj;
pub mod skinning;
pub mod types;
pub mod util;

// Top-level re-exports for common entry points and types
pub use gltf::load_gltf_mesh;
pub use obj::load_obj_mesh as load_obj_static;
pub use skinning::{load_gltf_skinned, merge_gltf_animations};
pub use types::{
    AnimClip, CpuMesh, SkinnedMeshCPU, TextureCPU, TrackQuat, TrackVec3, Vertex, VertexSkinCPU,
};
