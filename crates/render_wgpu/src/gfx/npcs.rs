//! NPC rings and instancing helpers.
//! Builds simple cube instances as targets across multiple rings and returns
//! the server state for use by zombie spawning.

use wgpu::util::DeviceExt;

fn find_clear_spawn(server: &server_core::ServerState, mut pos: glam::Vec3, my_radius: f32) -> glam::Vec3 {
    let pad = 0.5f32;
    let step = 3.0f32;
    for _ in 0..32 {
        let mut ok = true;
        for n in &server.npcs {
            let dx = n.pos.x - pos.x;
            let dz = n.pos.z - pos.z;
            let d2 = dx * dx + dz * dz;
            let min_d = my_radius + n.radius + pad;
            if d2 < min_d * min_d {
                ok = false;
                break;
            }
        }
        if ok { return pos; }
        pos.z += step;
    }
    pos
}

fn push_others_from(server: &mut server_core::ServerState, boss: server_core::NpcId, pad: f32) {
    let (bx, bz, br) = if let Some(b) = server.npcs.iter().find(|n| n.id == boss) {
        (b.pos.x, b.pos.z, b.radius)
    } else {
        return;
    };
    for n in &mut server.npcs {
        if n.id == boss { continue; }
        let mut dx = n.pos.x - bx;
        let mut dz = n.pos.z - bz;
        let d2 = dx * dx + dz * dz;
        let min_d = br + n.radius + pad;
        if d2 < min_d * min_d {
            let mut d = d2.sqrt();
            if d < 1e-4 { dx = 1.0; dz = 0.0; d = 1e-4; }
            dx /= d; dz /= d;
            let push = min_d - d;
            n.pos.x += dx * push;
            n.pos.z += dz * push;
        }
    }
}

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
    // Keep the close/mid zombie rings; drop the extreme far ring that caused
    // distant floating health bars.
    let near_count = 8usize; // was 10
    let near_radius = 15.0f32;
    let mid1_count = 12usize; // was 16
    let mid1_radius = 30.0f32;
    let mid2_count = 15usize; // was 20
    let mid2_radius = 45.0f32;
    let mid3_count = 18usize; // was 24
    let mid3_radius = 60.0f32;
    let far_count = 0usize; // remove far ring entirely
    let _far_radius = terrain_extent * 0.7;
    // Spawn rings (hp scales mildly with distance)
    server.ring_spawn(near_count, near_radius, 20);
    server.ring_spawn(mid1_count, mid1_radius, 25);
    server.ring_spawn(mid2_count, mid2_radius, 30);
    server.ring_spawn(mid3_count, mid3_radius, 35);

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

    log::debug!(
        "spawned {} NPCs across rings: near={}, mid1={}, mid2={}, mid3={}, far={}",
        server.npcs.len(),
        near_count,
        mid1_count,
        mid2_count,
        mid3_count,
        far_count
    );

    // Spawn the unique boss Nivita at a clear location (avoid zombie rings)
    // The renderer visual will follow server-authoritative position.
    let desired = glam::vec3(0.0, 0.6, 35.0);
    let clear = find_clear_spawn(&server, desired, 0.9);
    if let Some(id) = server.spawn_nivita_unique(clear) {
        // Immediately push away nearby NPCs to avoid initial overlap
        push_others_from(&mut server, id, 0.5);
    }

    NpcGpu {
        vb,
        ib,
        index_count,
        instances,
        models,
        server,
    }
}
