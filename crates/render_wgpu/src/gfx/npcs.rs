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
    pub server: server_core::ServerState,
}

pub fn build(device: &wgpu::Device, terrain_extent: f32) -> NpcGpu {
    let (vb, ib, index_count) = super::mesh::create_cube(device);
    let mut server = server_core::ServerState::new();
    // Reduce zombies ~25% overall by lowering ring counts
    let near_count = 8usize; // was 10
    let near_radius = 15.0f32;
    let mid1_count = 12usize; // was 16
    let mid1_radius = 30.0f32;
    let mid2_count = 15usize; // was 20
    let mid2_radius = 45.0f32;
    let mid3_count = 18usize; // was 24
    let mid3_radius = 60.0f32;
    let far_count = 9usize; // was 12
    let far_radius = terrain_extent * 0.7;
    // Spawn rings (hp scales mildly with distance)
    server.ring_spawn(near_count, near_radius, 20);
    server.ring_spawn(mid1_count, mid1_radius, 25);
    server.ring_spawn(mid2_count, mid2_radius, 30);
    server.ring_spawn(mid3_count, mid3_radius, 35);
    server.ring_spawn(far_count, far_radius, 30);

    let mut instances_cpu: Vec<super::types::Instance> = Vec::new();
    let mut models: Vec<glam::Mat4> = Vec::new();
    for npc in &server.npcs {
        let m = glam::Mat4::from_scale_rotation_translation(
            glam::Vec3::splat(1.2),
            glam::Quat::IDENTITY,
            npc.pos,
        );
        models.push(m);
        instances_cpu.push(super::types::Instance {
            model: m.to_cols_array_2d(),
            color: [0.75, 0.2, 0.2],
            selected: 0.0,
        });
    }
    let instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("npc-instances"),
        contents: bytemuck::cast_slice(&instances_cpu),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    });

    log::info!(
        "spawned {} NPCs across rings: near={}, mid1={}, mid2={}, mid3={}, far={}",
        server.npcs.len(),
        near_count,
        mid1_count,
        mid2_count,
        mid3_count,
        far_count
    );

    NpcGpu {
        vb,
        ib,
        index_count,
        instances,
        models,
        server,
    }
}
