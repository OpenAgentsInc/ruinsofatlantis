//! FX helpers: projectile/particle resources and update/upload.

use crate::gfx::types::ParticleInstance;
use wgpu::util::DeviceExt;

#[derive(Clone, Copy, Debug)]
pub struct Projectile {
    pub pos: glam::Vec3,
    pub vel: glam::Vec3,
    pub t_die: f32,
    pub owner_wizard: Option<usize>,
    pub color: [f32; 3],
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct Particle {
    pub pos: glam::Vec3,
    pub vel: glam::Vec3,
    pub age: f32,
    pub life: f32,
    pub size: f32,
    pub color: [f32; 3],
}

pub struct FxResources {
    pub instances: wgpu::Buffer,
    pub capacity: u32,
    pub model_bg: wgpu::BindGroup,
    pub quad_vb: wgpu::Buffer,
}

pub fn create_fx_resources(
    device: &wgpu::Device,
    model_bgl: &wgpu::BindGroupLayout,
) -> FxResources {
    let fx_capacity = 2048u32;
    let instances = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("fx-instances"),
        size: (fx_capacity as usize * std::mem::size_of::<ParticleInstance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    // FX model (bright emissive)
    #[repr(C)]
    #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
    struct Model {
        model: [[f32; 4]; 4],
        color: [f32; 3],
        emissive: f32,
        _pad: [f32; 4],
    }
    let model = Model {
        model: glam::Mat4::IDENTITY.to_cols_array_2d(),
        color: [1.0, 0.7, 0.2],
        emissive: 1.0,
        _pad: [0.0; 4],
    };
    let model_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("fx-model"),
        contents: bytemuck::bytes_of(&model),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });
    let model_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("fx-model-bg"),
        layout: model_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: model_buf.as_entire_binding(),
        }],
    });
    // Static unit quad for particles (triangle strip)
    #[repr(C)]
    #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
    struct ParticleVertex {
        corner: [f32; 2],
    }
    let quad: [ParticleVertex; 4] = [
        ParticleVertex {
            corner: [-0.5, -0.5],
        },
        ParticleVertex {
            corner: [0.5, -0.5],
        },
        ParticleVertex {
            corner: [-0.5, 0.5],
        },
        ParticleVertex { corner: [0.5, 0.5] },
    ];
    let quad_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("quad-vb"),
        contents: bytemuck::cast_slice(&quad),
        usage: wgpu::BufferUsages::VERTEX,
    });
    FxResources {
        instances,
        capacity: fx_capacity,
        model_bg,
        quad_vb,
    }
}

// (integration/upload helper removed; handled inline by renderer for now)
