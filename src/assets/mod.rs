//! Asset system (CPU-side) for loading meshes (module root).
//!
//! Submodules:
//! - `types`: CPU data structures (meshes, tracks, textures)
//! - `gltf`: unskinned GLTF mesh loading (with JSON+Draco fallback)
//! - `skinning`: skinned mesh + animation clip loading
//! - `draco`: Draco decode helpers (internal)
//! - `util`: path helpers and policy enforcement

pub mod types;
pub mod gltf;
pub mod skinning;
mod draco;
pub mod util;

pub use types::{
    AnimClip, CpuMesh, SkinnedMeshCPU, TextureCPU, TrackQuat, TrackVec3, VertexSkinCPU,
};
pub use gltf::load_gltf_mesh;
pub use skinning::load_gltf_skinned;
pub use util::prepare_gltf_path;

