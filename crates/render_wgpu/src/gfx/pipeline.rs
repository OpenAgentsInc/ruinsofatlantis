//! Pipeline creation helpers and shader loading.
//!
//! Layout
//! - Core forward pipelines: non‑instanced (terrain), instanced (static meshes), and
//!   skinned‑instanced (wizards/zombies).
//! - Post: sky (background), present (fog + tonemap + grade), blit (SceneColor→SceneRead),
//!   optional SSGI/SSR/AO overlays, and a simple bloom add pass.
//!
//! The post chain is designed to be modular; each pass consumes and produces color on
//! either the offscreen HDR `SceneColor` or the transient `SceneRead` texture.
//!
//! WGSL source lives in `shader.wgsl` next to this file and is embedded at compile time
//! with `include_str!` for convenience (no runtime file IO).

use wgpu::{
    BindGroupLayout, ColorTargetState, FragmentState, PipelineLayoutDescriptor, PolygonMode,
    RenderPipeline, ShaderModule, ShaderSource, VertexState,
};

use crate::gfx::types::{
    Instance, InstanceSkin, ParticleInstance, ParticleVertex, Vertex, VertexPosUv, VertexSkinned,
};

pub fn create_shader(device: &wgpu::Device) -> ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("basic-shader"),
        source: ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!("shader.wgsl"))),
    })
}

pub fn create_bind_group_layouts(device: &wgpu::Device) -> (BindGroupLayout, BindGroupLayout) {
    // Globals (view/proj + time) + Lights (packed UBO)
    let globals = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("globals-bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    // Per-draw Model
    let model = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("model-bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });

    (globals, model)
}

// Note: lights UBO is packed into the Globals bind group (binding=1) to stay under
// max_bind_groups across pipelines. No separate lights bind group is used.

pub fn create_palettes_bgl(device: &wgpu::Device) -> BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("palettes-bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    })
}

pub fn create_material_bgl(device: &wgpu::Device) -> BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("material-bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}

pub fn create_pipelines(
    device: &wgpu::Device,
    shader: &ShaderModule,
    globals_bgl: &BindGroupLayout,
    model_bgl: &BindGroupLayout,
    color_format: wgpu::TextureFormat,
) -> (RenderPipeline, RenderPipeline, Option<RenderPipeline>) {
    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("pipeline-layout"),
        bind_group_layouts: &[globals_bgl, model_bgl],
        push_constant_ranges: &[],
    });

    let depth_format = wgpu::TextureFormat::Depth32Float;

    // Non-instanced pipeline (plane)
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("pipeline"),
        layout: Some(&pipeline_layout),
        vertex: VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[Vertex::LAYOUT],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            targets: &[Some(ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: depth_format,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    // Instanced pipeline (adds per-instance buffer)
    let inst_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("inst-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: VertexState {
            module: shader,
            entry_point: Some("vs_inst"),
            buffers: &[Vertex::LAYOUT, Instance::LAYOUT],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: shader,
            entry_point: Some("fs_inst"),
            targets: &[Some(ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: depth_format,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    // Optional wireframe
    let wire_pipeline = if device
        .features()
        .contains(wgpu::Features::POLYGON_MODE_LINE)
    {
        Some(
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("inst-pipeline-wire"),
                layout: Some(&pipeline_layout),
                vertex: VertexState {
                    module: shader,
                    entry_point: Some("vs_inst"),
                    buffers: &[Vertex::LAYOUT, Instance::LAYOUT],
                    compilation_options: Default::default(),
                },
                fragment: Some(FragmentState {
                    module: shader,
                    entry_point: Some("fs_inst"),
                    targets: &[Some(ColorTargetState {
                        format: color_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    polygon_mode: PolygonMode::Line,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: depth_format,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            }),
        )
    } else {
        None
    };

    (pipeline, inst_pipeline, wire_pipeline)
}

pub fn create_textured_inst_pipeline(
    device: &wgpu::Device,
    shader: &ShaderModule,
    globals_bgl: &BindGroupLayout,
    model_bgl: &BindGroupLayout,
    palettes_bgl: &BindGroupLayout,
    material_bgl: &BindGroupLayout,
    color_format: wgpu::TextureFormat,
) -> RenderPipeline {
    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("inst-tex-pipeline-layout"),
        // Keep indices aligned with shader expectations:
        // 0 = globals, 1 = model, 2 = palettes (unused in this pipeline), 3 = material
        bind_group_layouts: &[globals_bgl, model_bgl, palettes_bgl, material_bgl],
        push_constant_ranges: &[],
    });
    let depth_format = wgpu::TextureFormat::Depth32Float;
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("inst-tex-pipeline"),
        layout: Some(&layout),
        vertex: VertexState {
            module: shader,
            entry_point: Some("vs_inst_tex"),
            buffers: &[
                crate::gfx::types::VertexPosNrmUv::LAYOUT,
                crate::gfx::types::Instance::LAYOUT,
            ],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: shader,
            entry_point: Some("fs_inst_tex"),
            targets: &[Some(ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: depth_format,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

pub fn create_textured_inst_ghost_pipeline(
    device: &wgpu::Device,
    shader: &ShaderModule,
    globals_bgl: &BindGroupLayout,
    model_bgl: &BindGroupLayout,
    palettes_bgl: &BindGroupLayout,
    material_bgl: &BindGroupLayout,
    color_format: wgpu::TextureFormat,
) -> RenderPipeline {
    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("inst-tex-ghost-pipeline-layout"),
        bind_group_layouts: &[globals_bgl, model_bgl, palettes_bgl, material_bgl],
        push_constant_ranges: &[],
    });
    let depth_format = wgpu::TextureFormat::Depth32Float;
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("inst-tex-ghost-pipeline"),
        layout: Some(&layout),
        vertex: VertexState {
            module: shader,
            entry_point: Some("vs_inst_tex"),
            buffers: &[
                crate::gfx::types::VertexPosNrmUv::LAYOUT,
                crate::gfx::types::Instance::LAYOUT,
            ],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: shader,
            entry_point: Some("fs_inst_tex_ghost"),
            targets: &[Some(ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: depth_format,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

// Sky background pipeline (fullscreen triangle)
pub fn create_sky_bgl(device: &wgpu::Device) -> BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("sky-bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    })
}

pub fn create_sky_pipeline(
    device: &wgpu::Device,
    globals_bgl: &BindGroupLayout,
    sky_bgl: &BindGroupLayout,
    color_format: wgpu::TextureFormat,
) -> RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("sky-shader"),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!("sky.wgsl"))),
    });
    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("sky-pipeline-layout"),
        bind_group_layouts: &[globals_bgl, sky_bgl],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("sky-pipeline"),
        layout: Some(&layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_sky"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: &shader,
            entry_point: Some("fs_sky"),
            targets: &[Some(ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn create_wizard_pipelines(
    device: &wgpu::Device,
    shader: &ShaderModule,
    globals_bgl: &BindGroupLayout,
    model_bgl: &BindGroupLayout,
    palettes_bgl: &BindGroupLayout,
    material_bgl: &BindGroupLayout,
    color_format: wgpu::TextureFormat,
) -> (RenderPipeline, Option<RenderPipeline>) {
    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("wizard-pipeline-layout"),
        bind_group_layouts: &[globals_bgl, model_bgl, palettes_bgl, material_bgl],
        push_constant_ranges: &[],
    });

    let depth_format = wgpu::TextureFormat::Depth32Float;
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("wizard-inst-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: VertexState {
            module: shader,
            entry_point: Some("vs_wizard"),
            buffers: &[VertexSkinned::LAYOUT, InstanceSkin::LAYOUT],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: shader,
            entry_point: Some("fs_wizard"),
            targets: &[Some(ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: depth_format,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let wire_pipeline = if device
        .features()
        .contains(wgpu::Features::POLYGON_MODE_LINE)
    {
        Some(
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("wizard-inst-pipeline-wire"),
                layout: Some(&pipeline_layout),
                vertex: VertexState {
                    module: shader,
                    entry_point: Some("vs_wizard"),
                    buffers: &[VertexSkinned::LAYOUT, InstanceSkin::LAYOUT],
                    compilation_options: Default::default(),
                },
                fragment: Some(FragmentState {
                    module: shader,
                    entry_point: Some("fs_wizard"),
                    targets: &[Some(ColorTargetState {
                        format: color_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    polygon_mode: PolygonMode::Line,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: depth_format,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            }),
        )
    } else {
        None
    };

    (pipeline, wire_pipeline)
}

#[allow(clippy::too_many_arguments)]
pub fn create_wizard_pipeline_debug(
    device: &wgpu::Device,
    shader: &ShaderModule,
    globals_bgl: &BindGroupLayout,
    model_bgl: &BindGroupLayout,
    palettes_bgl: &BindGroupLayout,
    material_bgl: &BindGroupLayout,
    color_format: wgpu::TextureFormat,
) -> RenderPipeline {
    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("wizard-pipeline-layout-debug"),
        bind_group_layouts: &[globals_bgl, model_bgl, palettes_bgl, material_bgl],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("wizard-inst-pipeline-debug"),
        layout: Some(&pipeline_layout),
        vertex: VertexState {
            module: shader,
            entry_point: Some("vs_wizard"),
            buffers: &[VertexSkinned::LAYOUT, InstanceSkin::LAYOUT],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: shader,
            entry_point: Some("fs_wizard_debug_flat"),
            targets: &[Some(ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

pub fn create_particle_pipeline(
    device: &wgpu::Device,
    shader: &ShaderModule,
    globals_bgl: &BindGroupLayout,
    color_format: wgpu::TextureFormat,
) -> RenderPipeline {
    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("particle-pipeline-layout"),
        bind_group_layouts: &[globals_bgl],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("particle-pipeline"),
        layout: Some(&layout),
        vertex: VertexState {
            module: shader,
            entry_point: Some("vs_particle"),
            buffers: &[ParticleVertex::LAYOUT, ParticleInstance::LAYOUT],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: shader,
            entry_point: Some("fs_particle"),
            targets: &[Some(ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            strip_index_format: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

// Text rendering: simple textured quad in screen space (NDC in vertex positions)
pub fn create_text_bgl(device: &wgpu::Device) -> BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("text-bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

pub fn create_text_pipeline(
    device: &wgpu::Device,
    shader: &ShaderModule,
    text_bgl: &BindGroupLayout,
    color_format: wgpu::TextureFormat,
) -> RenderPipeline {
    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("text-pipeline-layout"),
        bind_group_layouts: &[text_bgl],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("text-pipeline"),
        layout: Some(&layout),
        vertex: VertexState {
            module: shader,
            entry_point: Some("vs_text"),
            buffers: &[crate::gfx::types::TextVertex::LAYOUT],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: shader,
            entry_point: Some("fs_text"),
            targets: &[Some(ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

// Health bar pipeline (screen-space solid-color quads)
pub fn create_bar_pipeline(
    device: &wgpu::Device,
    shader: &ShaderModule,
    color_format: wgpu::TextureFormat,
) -> RenderPipeline {
    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("bar-pipeline-layout"),
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("bar-pipeline"),
        layout: Some(&layout),
        vertex: VertexState {
            module: shader,
            entry_point: Some("vs_bar"),
            buffers: &[crate::gfx::types::BarVertex::LAYOUT],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: shader,
            entry_point: Some("fs_bar"),
            targets: &[Some(ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

// Present (blit/tonemap) pipeline from SceneColor to swapchain
pub fn create_present_bgl(device: &wgpu::Device) -> BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("present-bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    // Allow binding HDR offscreen formats that are not filterable on WebGPU
                    // (e.g., Rgba16Float). Sampling uses a non‑filtering sampler.
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                // Use a NonFiltering sampler so we can sample non‑filterable formats.
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                count: None,
            },
            // Depth texture for fog (sampled as depth)
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Depth,
                },
                count: None,
            },
            // Non-filtering sampler used when sampling depth
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                count: None,
            },
        ],
    })
}

pub fn create_present_pipeline(
    device: &wgpu::Device,
    globals_bgl: &BindGroupLayout,
    present_bgl: &BindGroupLayout,
    color_format: wgpu::TextureFormat,
) -> RenderPipeline {
    // Compose shared fullscreen VS (with present Y flip) + present FS
    let src = [
        include_str!("fullscreen.wgsl"),
        include_str!("present.wgsl"),
    ]
    .join("\n\n");
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("present-shader"),
        source: ShaderSource::Wgsl(std::borrow::Cow::Owned(src)),
    });
    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("present-pipeline-layout"),
        bind_group_layouts: &[globals_bgl, present_bgl],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("present-pipeline"),
        layout: Some(&layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_fullscreen_present_flip"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: &shader,
            entry_point: Some("fs_present"),
            targets: &[Some(ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

// Blit pipeline (no Y flip) used for copying SceneColor -> SceneRead
pub fn create_blit_pipeline(
    device: &wgpu::Device,
    present_bgl: &BindGroupLayout,
    color_format: wgpu::TextureFormat,
) -> RenderPipeline {
    // Compose shared fullscreen VS (no flip) + blit FS
    let src = [
        include_str!("fullscreen.wgsl"),
        include_str!("blit_noflip.wgsl"),
    ]
    .join("\n\n");
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("blit-noflip-shader"),
        source: ShaderSource::Wgsl(std::borrow::Cow::Owned(src)),
    });
    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("blit-noflip-pipeline-layout"),
        bind_group_layouts: &[present_bgl],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("blit-noflip-pipeline"),
        layout: Some(&layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_fullscreen_noflip"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: &shader,
            entry_point: Some("fs_blit"),
            targets: &[Some(ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

// Bloom pipeline: threshold + small blur (single pass) additive into SceneColor
pub fn create_bloom_bgl(device: &wgpu::Device) -> BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("bloom-bgl"),
        entries: &[
            // Scene source
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

pub fn create_bloom_pipeline(
    device: &wgpu::Device,
    bloom_bgl: &BindGroupLayout,
    color_format: wgpu::TextureFormat,
) -> RenderPipeline {
    let src = [
        include_str!("fullscreen.wgsl"),
        include_str!("post_bloom.wgsl"),
    ]
    .join("\n\n");
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("bloom-shader"),
        source: ShaderSource::Wgsl(std::borrow::Cow::Owned(src)),
    });
    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("bloom-pipeline-layout"),
        bind_group_layouts: &[bloom_bgl],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("bloom-pipeline"),
        layout: Some(&layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_fullscreen_noflip"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: &shader,
            entry_point: Some("fs_bloom"),
            targets: &[Some(ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent::REPLACE,
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

// Frame overlay (alpha blended) into SceneColor to visualize frame progression
// Frame overlay disabled

// Post-process AO pipeline (fullscreen triangle sampling depth)
pub fn create_post_ao_bgl(device: &wgpu::Device) -> BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("post-ao-bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Depth,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                // Depth textures must not use a filtering sampler with textureSample.
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                count: None,
            },
        ],
    })
}

pub fn create_post_ao_pipeline(
    device: &wgpu::Device,
    globals_bgl: &BindGroupLayout,
    post_ao_bgl: &BindGroupLayout,
    color_format: wgpu::TextureFormat,
) -> RenderPipeline {
    // Compose shared fullscreen VS (no flip) + AO FS
    let src = [
        include_str!("fullscreen.wgsl"),
        include_str!("post_ao.wgsl"),
    ]
    .join("\n\n");
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("post-ao-shader"),
        source: ShaderSource::Wgsl(std::borrow::Cow::Owned(src)),
    });
    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("post-ao-pipeline-layout"),
        bind_group_layouts: &[globals_bgl, post_ao_bgl],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("post-ao-pipeline"),
        layout: Some(&layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_fullscreen_noflip"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: &shader,
            entry_point: Some("fs_ao"),
            targets: &[Some(ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        // dst.rgb = dst.rgb * src.rgb  (src = AO term)
                        src_factor: wgpu::BlendFactor::Zero,
                        dst_factor: wgpu::BlendFactor::Src,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        // keep alpha unchanged
                        src_factor: wgpu::BlendFactor::Zero,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::RED
                    | wgpu::ColorWrites::GREEN
                    | wgpu::ColorWrites::BLUE,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

// SSGI additive overlay (fullscreen), samples depth + scene color
pub fn create_ssgi_bgl(
    device: &wgpu::Device,
) -> (BindGroupLayout, BindGroupLayout, BindGroupLayout) {
    let globals = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("ssgi-globals-bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let depth = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("ssgi-depth-bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Depth,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                // Non-filtering sampler when sampling depth without comparison.
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                count: None,
            },
        ],
    });
    let scene = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("ssgi-scene-bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    (globals, depth, scene)
}

// SSR overlays (fullscreen), samples linear depth (R32F, mip chain) + scene color
pub fn create_ssr_bgl(device: &wgpu::Device) -> (BindGroupLayout, BindGroupLayout) {
    let depth = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("ssr-depth-bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    // Linear depth is R32Float: non-filterable
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                // Must bind a non-filtering sampler for R32Float
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                count: None,
            },
        ],
    });
    let scene = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("ssr-scene-bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    (depth, scene)
}

pub fn create_ssr_pipeline(
    device: &wgpu::Device,
    depth_bgl: &BindGroupLayout,
    scene_bgl: &BindGroupLayout,
    color_format: wgpu::TextureFormat,
) -> RenderPipeline {
    let src = [include_str!("fullscreen.wgsl"), include_str!("ssr_fs.wgsl")].join("\n\n");
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("ssr-fs-shader"),
        source: ShaderSource::Wgsl(std::borrow::Cow::Owned(src)),
    });
    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("ssr-fs-pipeline-layout"),
        bind_group_layouts: &[depth_bgl, scene_bgl],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("ssr-fs-pipeline"),
        layout: Some(&layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_fullscreen_noflip"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: &shader,
            entry_point: Some("fs_ssr"),
            targets: &[Some(ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

pub fn create_ssgi_pipeline(
    device: &wgpu::Device,
    globals_bgl: &BindGroupLayout,
    depth_bgl: &BindGroupLayout,
    scene_bgl: &BindGroupLayout,
    color_format: wgpu::TextureFormat,
) -> RenderPipeline {
    // Compose shared fullscreen VS (no flip) + SSGI FS
    let src = [
        include_str!("fullscreen.wgsl"),
        include_str!("ssgi_fs.wgsl"),
    ]
    .join("\n\n");
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("ssgi-fs-shader"),
        source: ShaderSource::Wgsl(std::borrow::Cow::Owned(src)),
    });
    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("ssgi-fs-pipeline-layout"),
        bind_group_layouts: &[globals_bgl, depth_bgl, scene_bgl],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("ssgi-fs-pipeline"),
        layout: Some(&layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_fullscreen_noflip"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: &shader,
            entry_point: Some("fs_ssgi"),
            targets: &[Some(ColorTargetState {
                format: color_format,
                // Additive blend: dst = dst + src
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

#[allow(dead_code)]
pub fn create_wizard_simple_pipeline(
    device: &wgpu::Device,
    globals_bgl: &BindGroupLayout,
    material_bgl: &BindGroupLayout,
    color_format: wgpu::TextureFormat,
) -> RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("wizard-simple-shader"),
        source: ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
            "shader_wizard_viewer.wgsl"
        ))),
    });
    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("wizard-simple-pipeline-layout"),
        bind_group_layouts: &[globals_bgl, material_bgl],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("wizard-simple-pipeline"),
        layout: Some(&layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[VertexPosUv::LAYOUT, Instance::LAYOUT],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}
