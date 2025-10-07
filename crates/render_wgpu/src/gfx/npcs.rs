//! NPC rings and instancing helpers.
//! Builds simple cube instances as targets across multiple rings and returns
//! the server state for use by zombie spawning.

use wgpu::util::DeviceExt;

pub struct NpcGpu {
    pub vb: wgpu::Buffer,
    pub ib: wgpu::Buffer,
    pub index_count: u32,
    pub instances: wgpu::Buffer,
    pub models: Vec<glam::Mat4>,
}

pub fn build(device: &wgpu::Device, terrain_extent: f32) -> NpcGpu {
    let (vb, ib, index_count) = super::mesh::create_cube(device);
    // Keep the close/mid zombie rings; drop the extreme far ring that caused
    // distant floating health bars.
    let _near_count = 8usize; // was 10
    let _near_radius = 15.0f32;
    let _mid1_count = 12usize; // was 16
    let _mid1_radius = 30.0f32;
    let _mid2_count = 15usize; // was 20
    let _mid2_radius = 45.0f32;
    let _mid3_count = 18usize; // was 24
    let _mid3_radius = 60.0f32;
    let _far_count = 0usize; // remove far ring entirely
    let _far_radius = terrain_extent * 0.7;
    // Spawn rings (hp scales mildly with distance)
    // Rings spawned by server authority in demo; client-only visuals remain empty here.

    let instances_cpu: Vec<super::types::Instance> = Vec::new();
    let models: Vec<glam::Mat4> = Vec::new();
    // Legacy client NPC cubes removed — visuals are driven by replication/zombie instances.
    let instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("npc-instances"),
        contents: bytemuck::cast_slice(&instances_cpu),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    });

    // Logging removed: NPCs are no longer spawned client-side.

    // Note: Server authority (unique boss spawn) is handled outside the renderer.
    // The renderer should remain presentation-only and consume server state.

    NpcGpu {
        vb,
        ib,
        index_count,
        instances,
        models,
    }
}
