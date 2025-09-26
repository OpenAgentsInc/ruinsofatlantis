//! Asset type definitions used across loaders.
//!
//! These are CPU-side representations independent of any renderer.

use glam::{Mat4, Quat, Vec3};
use std::collections::HashMap;

/// Minimal vertex with position and normal.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub nrm: [f32; 3],
}

/// CPU-side mesh ready to be uploaded to GPU.
pub struct CpuMesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
}

#[derive(Clone)]
pub struct VertexSkinCPU {
    pub pos: [f32; 3],
    pub nrm: [f32; 3],
    pub joints: [u16; 4],
    pub weights: [f32; 4],
    pub uv: [f32; 2],
}

#[derive(Clone)]
pub struct TrackVec3 {
    pub times: Vec<f32>,
    pub values: Vec<Vec3>,
}

#[derive(Clone)]
pub struct TrackQuat {
    pub times: Vec<f32>,
    pub values: Vec<Quat>,
}

#[derive(Clone)]
pub struct AnimClip {
    pub name: String,
    pub duration: f32,
    pub t_tracks: HashMap<usize, TrackVec3>,
    pub r_tracks: HashMap<usize, TrackQuat>,
    pub s_tracks: HashMap<usize, TrackVec3>,
}

pub struct TextureCPU {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub srgb: bool,
}

pub struct SkinnedMeshCPU {
    pub vertices: Vec<VertexSkinCPU>,
    pub indices: Vec<u16>,
    pub joints_nodes: Vec<usize>,
    pub inverse_bind: Vec<Mat4>,
    pub parent: Vec<Option<usize>>, // node parent map
    pub base_t: Vec<Vec3>,
    pub base_r: Vec<Quat>,
    pub base_s: Vec<Vec3>,
    pub animations: HashMap<String, AnimClip>,
    pub base_color_texture: Option<TextureCPU>,
    pub node_names: Vec<String>,
    pub hand_right_node: Option<usize>,
    pub root_node: Option<usize>,
}
