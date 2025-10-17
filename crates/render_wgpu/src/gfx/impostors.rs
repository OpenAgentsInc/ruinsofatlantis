//! Octahedral impostor demo (first pass)
//!
//! Scope
//! - Minimal pipeline that draws instanced camera-facing quads using a 2D array texture
//! - Loads layers from `assets/horde/octa_demo/albedo/*.png` (sorted by filename)
//! - Falls back to procedurally generated colored layers if no files are present
//! - Sets up the infrastructure to evolve into true octahedral impostors
//!
//! Extending
//! - Add octahedral mapping (dir→UV tile) and grid metadata
//! - Add normal/depth arrays and alpha-to-coverage for edges
//! - GPU culling and compute-driven simulation buffers

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ImpostorInst {
    pos: [f32; 3],
    yaw: f32,
    layer: u32,
    scale: f32,
}

pub struct ImpostorDemo {
    pipeline: wgpu::RenderPipeline,
    bg: wgpu::BindGroup,
    params_buf: wgpu::Buffer,
    inst: wgpu::Buffer,
    count: u32,
    sps: u32,
    pal_size: u32,
    pal_rows: u32,
    use_palette: bool,
    variant: u32,
    fps: f32,
    // CPU state for simple wander movement
    inst_cpu: Vec<ImpostorInst>,
    vel_cpu: Vec<[f32; 2]>, // xz planar velocities
    prev_time: f32,
}

impl ImpostorDemo {
    pub fn new(r: &crate::gfx::Renderer) -> Result<Self> {
        let device = &r.device;
        let format = if r.direct_present {
            r.config.format
        } else {
            r.attachments.offscreen_format
        };

        // Bind group layout: globals @0 (from renderer), impostor material @1
        let mat_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("impostor-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    // texture array
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    // sampler
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    // palette texture (optional)
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    // palette sampler
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    // params uniform
                    binding: 4,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(64),
                    },
                    count: None,
                },
            ],
        });

        // Pipeline layout reuses renderer globals at set=0
        // Use the same globals layout as the main pipeline (set=0)
        let globals_bgl = r.pipeline.get_bind_group_layout(0);
        let pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("impostor-pipeline-layout"),
            bind_group_layouts: &[&globals_bgl, &mat_bgl],
            push_constant_ranges: &[],
        });

        // Shader module (reuses existing main shader with new entry points)
        let sm = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader-impostor"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        // Vertex layouts: slot 0 = quad corner (vec2), slot 1 = instance
        let inst_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ImpostorInst>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    shader_location: 1,
                    offset: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    shader_location: 2,
                    offset: 12,
                    format: wgpu::VertexFormat::Float32,
                },
                wgpu::VertexAttribute {
                    shader_location: 3,
                    offset: 16,
                    format: wgpu::VertexFormat::Uint32,
                },
                wgpu::VertexAttribute {
                    shader_location: 4,
                    offset: 20,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        };

        let pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("impostor-pipeline"),
            layout: Some(&pipe_layout),
            vertex: wgpu::VertexState {
                module: &sm,
                entry_point: Some("vs_impostor"),
                compilation_options: Default::default(),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<[f32; 2]>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[wgpu::VertexAttribute {
                            shader_location: 0,
                            offset: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        }],
                    },
                    inst_layout,
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &sm,
                entry_point: Some("fs_impostor"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Try to load texture array from assets; fallback to generated layers
        let (view, samp, layers) = load_or_generate_array(device, &r.queue)?;
        let (pal_view, pal_samp, pal_size, pal_rows, use_palette) = load_palette(device, &r.queue);
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct Params {
            sps: u32,
            use_palette: u32,
            pal_size: u32,
            pal_rows: u32,
            time: f32,
            fps: f32,
            alpha_clamp: f32,
            pad0: f32,
            cam_pos: [f32; 4],
            variant: u32,
            pad1: [u32; 3],
        }
        let params = Params {
            sps: 16,
            use_palette: if use_palette { 1 } else { 0 },
            pal_size: pal_size,
            pal_rows: pal_rows,
            time: 0.0,
            fps: 24.0,
            alpha_clamp: 0.02,
            pad0: 0.0,
            cam_pos: [0.0, 0.0, 0.0, 0.0],
            variant: 3,
            pad1: [0, 0, 0],
        };
        let params_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("impostor-params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("impostor-bg"),
            layout: &mat_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&samp),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&pal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&pal_samp),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: params_buf.as_entire_binding(),
                },
            ],
        });

        // Populate a small grid of instances
        let mut insts: Vec<ImpostorInst> = Vec::new();
        let mut vels: Vec<[f32; 2]> = Vec::new();
        let n = 32u32;
        let pitch = 2.2f32;
        for y in 0..n {
            for x in 0..n {
                let px = (x as f32 - n as f32 * 0.5) * pitch;
                let py = (y as f32 - n as f32 * 0.5) * pitch;
                insts.push(ImpostorInst {
                    pos: [px, 0.5, py],
                    yaw: 0.0,
                    layer: (x % layers) as u32,
                    scale: 1.6,
                });
                // Random-ish velocity field per instance
                let sx = ((x as i32 * 97 + y as i32 * 13) % 17) as f32 - 8.0;
                let sz = ((x as i32 * 23 + y as i32 * 41) % 19) as f32 - 9.0;
                let len = (sx * sx + sz * sz).sqrt().max(1.0);
                let s = 0.8; // m/s
                vels.push([s * sx / len, s * sz / len]);
            }
        }
        let inst_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("impostor-instances"),
            contents: bytemuck::cast_slice(&insts),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        Ok(Self {
            pipeline: pipe,
            bg,
            params_buf,
            inst: inst_buf,
            count: insts.len() as u32,
            sps: 16,
            pal_size,
            pal_rows,
            use_palette,
            variant: 3,
            fps: 24.0,
            inst_cpu: insts,
            vel_cpu: vels,
            prev_time: 0.0,
        })
    }

    pub fn draw<'rp>(
        &mut self,
        queue: &wgpu::Queue,
        globals_bg: &wgpu::BindGroup,
        quad_vb: &wgpu::Buffer,
        cam_pos: glam::Vec3,
        time: f32,
        rp: &mut wgpu::RenderPass<'rp>,
    ) {
        rp.set_pipeline(&self.pipeline);
        // set globals at set=0
        rp.set_bind_group(0, globals_bg, &[]);
        // Update camera position and timing params each frame
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct Params {
            sps: u32,
            use_palette: u32,
            pal_size: u32,
            pal_rows: u32,
            time: f32,
            fps: f32,
            alpha_clamp: f32,
            pad0: f32,
            cam_pos: [f32; 4],
            variant: u32,
            _pad: [u32; 3],
        }
        let cam = cam_pos;
        let time = time;
        let dt = (time - self.prev_time).clamp(0.0, 0.1);
        self.prev_time = time;
        // Update simple wander motion on CPU
        let half = 0.5 * (32.0 * 2.2); // grid half-size in meters
        for (i, inst) in self.inst_cpu.iter_mut().enumerate() {
            let v = self.vel_cpu[i];
            inst.pos[0] += v[0] * dt;
            inst.pos[2] += v[1] * dt;
            // Wrap around bounds to keep the field dense
            if inst.pos[0] > half {
                inst.pos[0] = -half;
            }
            if inst.pos[0] < -half {
                inst.pos[0] = half;
            }
            if inst.pos[2] > half {
                inst.pos[2] = -half;
            }
            if inst.pos[2] < -half {
                inst.pos[2] = half;
            }
        }
        // Upload updated instance positions
        queue.write_buffer(&self.inst, 0, bytemuck::cast_slice(&self.inst_cpu));
        let params = Params {
            sps: self.sps,
            use_palette: if self.use_palette { 1 } else { 0 },
            pal_size: self.pal_size,
            pal_rows: self.pal_rows,
            time,
            fps: self.fps,
            alpha_clamp: 0.02,
            pad0: 0.0,
            cam_pos: [cam.x, cam.y, cam.z, 0.0],
            variant: self.variant,
            _pad: [0, 0, 0],
        };
        queue.write_buffer(&self.params_buf, 0, bytemuck::bytes_of(&params));
        rp.set_bind_group(1, &self.bg, &[]);
        // slot0: quad corners (from renderer), slot1: instances
        rp.set_vertex_buffer(0, quad_vb.slice(..));
        rp.set_vertex_buffer(1, self.inst.slice(..));
        // triangle strip quad: use index buffer from renderer? We can draw 4 verts/strip with built-in index
        // Here we draw strip with an implicit index via set_index_buffer isn't required if shader constructs; but we can use a small dynamic index.
        // Simpler: issue a strip of 4 verts per instance using the quad_vb content (which matches particles pipeline)
        rp.draw(0..4, 0..self.count);
    }
}

fn load_or_generate_array(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(wgpu::TextureView, wgpu::Sampler, u32)> {
    // Prefer KTX2 2D array (R8 index) if available
    let ktx_candidates = [
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/horde/octa_demo/merged.ktx2"),
        PathBuf::from("/Users/christopherdavid/code/Horde/textures/merged.ktx2"),
    ];
    for p in &ktx_candidates {
        if p.exists() {
            if let Ok(ok) = load_ktx2_r8_array(device, queue, p) {
                return Ok(ok);
            }
        }
    }
    // Fallback: stacked PNG layers
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/horde/octa_demo/albedo");
    let mut files: Vec<PathBuf> = Vec::new();
    if base.exists() {
        for e in fs::read_dir(&base).with_context(|| format!("read_dir: {:?}", base))? {
            if let Ok(entry) = e {
                let p = entry.path();
                if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                    if matches!(ext.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg") {
                        files.push(p);
                    }
                }
            }
        }
        files.sort();
    }
    if files.is_empty() {
        // Generate a small 2D array: 8 layers of 64x64 colored squares
        let layers = 8u32;
        let (w, h) = (64u32, 64u32);
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("impostor-fallback-tex"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: layers,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for layer in 0..layers {
            let mut px = vec![0u8; (w * h * 4) as usize];
            let (r, g, b) = palette(layer as usize);
            for i in (0..px.len()).step_by(4) {
                px[i] = r;
                px[i + 1] = g;
                px[i + 2] = b;
                px[i + 3] = 255;
            }
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &px,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(w * 4),
                    rows_per_image: Some(h),
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
        }
        let view = tex.create_view(&wgpu::TextureViewDescriptor {
            label: Some("impostor-fallback-view"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("impostor-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        return Ok((view, sampler, layers));
    }

    // Load first to get dimensions
    let (w, h, layers) = {
        let img = image::open(&files[0])
            .context("open first impostor layer")?
            .to_rgba8();
        (img.width(), img.height(), files.len() as u32)
    };
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("impostor-tex"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: layers,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (zi, path) in files.iter().enumerate() {
        let img = image::open(path)
            .with_context(|| format!("open layer: {:?}", path))?
            .to_rgba8();
        let bytes = img.as_raw();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: zi as u32,
                },
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
    }
    let view = tex.create_view(&wgpu::TextureViewDescriptor {
        label: Some("impostor-view"),
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("impostor-sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    });
    Ok((view, sampler, layers))
}

fn palette(i: usize) -> (u8, u8, u8) {
    const P: &[(u8, u8, u8)] = &[
        (220, 80, 80),
        (80, 180, 220),
        (80, 200, 120),
        (200, 160, 80),
        (180, 120, 220),
        (220, 120, 120),
        (120, 200, 200),
        (200, 200, 120),
    ];
    P[i % P.len()]
}

fn load_ktx2_r8_array(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path: &Path,
) -> Result<(wgpu::TextureView, wgpu::Sampler, u32)> {
    use std::fs::File;
    use std::io::Read;
    let mut f = File::open(path).with_context(|| format!("open ktx2: {:?}", path))?;
    let mut data = Vec::new();
    f.read_to_end(&mut data)?;
    let reader = ktx2::Reader::new(&data).context("parse ktx2")?;
    let header = reader.header();
    let w = header.pixel_width;
    let h = header.pixel_height.max(1);
    let layers = header.layer_count.max(1);
    if let Some(fmt) = header.format {
        if fmt != ktx2::Format::R8_UNORM {
            log::warn!(
                "ktx2: expected R8_UNORM, got {:?}; proceeding as raw R8",
                fmt
            );
        }
    }
    let mut levels = reader.levels();
    let lvl0 = levels
        .next()
        .ok_or_else(|| anyhow::anyhow!("ktx2: missing level 0"))?;
    let bytes: Vec<u8> = match header.supercompression_scheme {
        Some(s) if s == ktx2::SupercompressionScheme::Zstandard => {
            zstd::stream::decode_all(&mut std::io::Cursor::new(lvl0.data))
                .context("zstd decompress")?
        }
        _ => lvl0.data.to_vec(),
    };
    let bpp = 1usize;
    let expected = (w as usize) * (h as usize) * (layers as usize) * bpp;
    if bytes.len() < expected {
        anyhow::bail!(
            "ktx2: level 0 too small: got {} expected {}",
            bytes.len(),
            expected
        );
    }
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("impostor-ktx2"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: layers,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let layer_stride = (w as usize) * (h as usize) * bpp;
    for z in 0..(layers as usize) {
        let start = z * layer_stride;
        let end = start + layer_stride;
        let slice = &bytes[start..end];
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: z as u32,
                },
                aspect: wgpu::TextureAspect::All,
            },
            slice,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
    }
    let view = tex.create_view(&wgpu::TextureViewDescriptor {
        label: Some("impostor-ktx2-view"),
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("impostor-index-sampler"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    });
    Ok((view, sampler, layers))
}

fn load_palette(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::TextureView, wgpu::Sampler, u32, u32, bool) {
    let candidates = [
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/horde/octa_demo/palette.png"),
        PathBuf::from("/Users/christopherdavid/code/Horde/textures/palette.png"),
    ];
    for p in &candidates {
        if p.exists() {
            if let Ok(img) = image::open(p) {
                let rgba = img.to_rgba8();
                let (w, h) = (rgba.width(), rgba.height());
                let tex = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("palette-tex"),
                    size: wgpu::Extent3d {
                        width: w,
                        height: h,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &tex,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &rgba,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(w * 4),
                        rows_per_image: Some(h),
                    },
                    wgpu::Extent3d {
                        width: w,
                        height: h,
                        depth_or_array_layers: 1,
                    },
                );
                let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
                let samp = device.create_sampler(&wgpu::SamplerDescriptor {
                    label: Some("palette-sampler"),
                    mag_filter: wgpu::FilterMode::Nearest,
                    min_filter: wgpu::FilterMode::Nearest,
                    mipmap_filter: wgpu::FilterMode::Nearest,
                    address_mode_u: wgpu::AddressMode::ClampToEdge,
                    address_mode_v: wgpu::AddressMode::ClampToEdge,
                    ..Default::default()
                });
                return (view, samp, w, h, true);
            }
        }
    }
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("palette-dummy"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
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
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    let samp = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("palette-sampler"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    });
    (view, samp, 1, 1, false)
}
