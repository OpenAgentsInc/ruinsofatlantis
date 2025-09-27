//! Material helpers for wizard rendering.
//!
//! Creates a bind group for the wizard’s base color texture and a small
//! material transform uniform (supports KHR_texture_transform if present).

use crate::assets::SkinnedMeshCPU;
use wgpu::util::DeviceExt;

pub struct WizardMaterial {
    pub bind_group: wgpu::BindGroup,
    pub uniform_buf: wgpu::Buffer,
    pub texture_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct MaterialXform {
    offset: [f32; 2],
    _pad0: [f32; 2], // std140 padding
    scale: [f32; 2],
    _pad1: [f32; 2], // std140 padding
    rot: f32,
    _pad2: [f32; 3], // std140 padding
}

pub fn create_wizard_material(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    material_bgl: &wgpu::BindGroupLayout,
    skinned_cpu: &SkinnedMeshCPU,
) -> WizardMaterial {
    let mat_xf = read_texture_transform().unwrap_or(MaterialXform {
        offset: [0.0, 0.0],
        _pad0: [0.0; 2],
        scale: [1.0, 1.0],
        _pad1: [0.0; 2],
        rot: 0.0,
        _pad2: [0.0; 3],
    });

    let wizard_mat_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("material-xform"),
        contents: bytemuck::bytes_of(&mat_xf),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let (bg, view, sampler) = if let Some(tex) = &skinned_cpu.base_color_texture {
        log::info!(
            "wizard albedo: {}x{} (srgb={})",
            tex.width,
            tex.height,
            tex.srgb
        );
        let size3 = wgpu::Extent3d {
            width: tex.width,
            height: tex.height,
            depth_or_array_layers: 1,
        };
        let tex_obj = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("wizard-albedo"),
            size: size3,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex_obj,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &tex.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * tex.width),
                rows_per_image: Some(tex.height),
            },
            size3,
        );
        let view = tex_obj.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("wizard-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            ..Default::default()
        });
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("wizard-material-bg"),
            layout: material_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wizard_mat_buf.as_entire_binding(),
                },
            ],
        });
        (bg, view, sampler)
    } else {
        log::warn!("wizard albedo: NONE; using 1x1 fallback");
        let size3 = wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        };
        let tex_obj = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("white-1x1"),
            size: size3,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex_obj,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            size3,
        );
        let view = tex_obj.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            ..Default::default()
        });
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("wizard-material-bg"),
            layout: material_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wizard_mat_buf.as_entire_binding(),
                },
            ],
        });
        (bg, view, sampler)
    };

    WizardMaterial {
        bind_group: bg,
        uniform_buf: wizard_mat_buf,
        texture_view: view,
        sampler,
    }
}

fn read_texture_transform() -> Option<MaterialXform> {
    // Read KHR_texture_transform from wizard.gltf (first primitive's material).
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/models/wizard.gltf");
    let txt = std::fs::read_to_string(&path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&txt).ok()?;
    let mat_index = json
        .get("meshes")?
        .get(0)?
        .get("primitives")?
        .get(0)?
        .get("material")?
        .as_u64()? as usize;
    let bct = json
        .get("materials")?
        .get(mat_index)?
        .get("pbrMetallicRoughness")?
        .get("baseColorTexture")?;
    let ext = bct.get("extensions")?.get("KHR_texture_transform")?;
    let mut xf = MaterialXform {
        offset: [0.0, 0.0],
        _pad0: [0.0; 2],
        scale: [1.0, 1.0],
        _pad1: [0.0; 2],
        rot: 0.0,
        _pad2: [0.0; 3],
    };
    if let Some(off) = ext.get("offset").and_then(|v| v.as_array())
        && off.len() == 2
    {
        xf.offset = [
            off[0].as_f64().unwrap_or(0.0) as f32,
            off[1].as_f64().unwrap_or(0.0) as f32,
        ];
    }
    if let Some(s) = ext.get("scale").and_then(|v| v.as_array())
        && s.len() == 2
    {
        xf.scale = [
            s[0].as_f64().unwrap_or(1.0) as f32,
            s[1].as_f64().unwrap_or(1.0) as f32,
        ];
    }
    if let Some(r) = ext.get("rotation").and_then(|v| v.as_f64()) {
        xf.rot = r as f32;
    }
    Some(xf)
}
