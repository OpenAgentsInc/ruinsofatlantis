//! Asset facade for the renderer crate.
//!
//! Re-exports from the shared `ra-assets` crate so existing paths like
//! `crate::assets::...` continue to work inside the renderer.

pub use ra_assets::gltf::load_gltf_mesh;
pub use ra_assets::load_obj_static as load_obj_mesh;
pub use ra_assets::skinning::{load_gltf_skinned, merge_gltf_animations};
pub use ra_assets::types::{
    AnimClip, CpuMesh, SkinnedMeshCPU, TextureCPU, TrackQuat, TrackVec3, Vertex, VertexSkinCPU,
};
pub use ra_assets::util::prepare_gltf_path;

pub mod skinning {
    pub use ra_assets::skinning::merge_gltf_animations;
}

