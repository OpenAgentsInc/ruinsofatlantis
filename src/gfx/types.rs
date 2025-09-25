//! Buffer/vertex types shared across pipelines.
//!
//! All types here are `#[repr(C)]` and `bytemuck`-safe so they can be uploaded to GPU buffers
//! without extra copies.

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Globals {
    pub view_proj: [[f32; 4]; 4],
    pub time_pad: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Model {
    pub model: [[f32; 4]; 4],
    pub color: [f32; 3],
    pub emissive: f32,
    pub _pad: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub nrm: [f32; 3],
}

impl Vertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
    };
}

// Skinned vertex: includes 4 joint indices (u16) and 4 weights (f32)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VertexSkinned {
    pub pos: [f32; 3],   // 0
    pub nrm: [f32; 3],   // 12
    pub uv: [f32; 2],    // 24
    pub joints: [u16; 4],// 32
    pub weights: [f32; 4],// 40
}

impl VertexSkinned {
    // Attribute locations shared with other pipelines:
    // 0=pos, 1=nrm, 2..7=instance (mat4,color,sel), 8=joints, 9=weights, 10=palette_base (instance)
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<VertexSkinned>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute { shader_location: 0, offset: 0, format: wgpu::VertexFormat::Float32x3 },
            wgpu::VertexAttribute { shader_location: 1, offset: 12, format: wgpu::VertexFormat::Float32x3 },
            wgpu::VertexAttribute { shader_location: 11, offset: 24, format: wgpu::VertexFormat::Float32x2 },
            wgpu::VertexAttribute { shader_location: 8, offset: 32, format: wgpu::VertexFormat::Uint16x4 },
            wgpu::VertexAttribute { shader_location: 9, offset: 40, format: wgpu::VertexFormat::Float32x4 },
        ],
    };
}

// Position + UV only (viewer-parity path)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VertexPosUv {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
}

impl VertexPosUv {
    #[allow(dead_code)]
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<VertexPosUv>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2],
    };
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Instance {
    pub model: [[f32; 4]; 4],
    pub color: [f32; 3],
    pub selected: f32,
}

impl Instance {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Instance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![
            2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4,
            6 => Float32x3, 7 => Float32
        ],
    };
}

// Skinned instance adds a palette base index (u32) to address into a storage buffer
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct InstanceSkin {
    pub model: [[f32; 4]; 4],
    pub color: [f32; 3],
    pub selected: f32,
    pub palette_base: u32,
    pub _pad_inst: [u32; 3],
}

impl InstanceSkin {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<InstanceSkin>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            // i0..i3 (mat4)
            wgpu::VertexAttribute { shader_location: 2, offset: 0,  format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { shader_location: 3, offset: 16, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { shader_location: 4, offset: 32, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { shader_location: 5, offset: 48, format: wgpu::VertexFormat::Float32x4 },
            // color + selected
            wgpu::VertexAttribute { shader_location: 6, offset: 64, format: wgpu::VertexFormat::Float32x3 },
            wgpu::VertexAttribute { shader_location: 7, offset: 76, format: wgpu::VertexFormat::Float32 },
            // palette base
            wgpu::VertexAttribute { shader_location: 10, offset: 80, format: wgpu::VertexFormat::Uint32 },
        ],
    };
}
