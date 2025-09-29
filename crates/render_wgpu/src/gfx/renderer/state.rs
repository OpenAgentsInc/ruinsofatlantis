//! Renderer state: struct and small enums, extracted from gfx/mod.rs.

use winit::dpi::PhysicalSize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PcCast {
    FireBolt,
    MagicMissile,
}

pub struct Renderer {
    // --- GPU & Surface ---
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) size: PhysicalSize<u32>,
    pub(crate) max_dim: u32,
    pub(crate) depth: wgpu::TextureView,
    // Offscreen scene color
    pub(crate) scene_color: wgpu::Texture,
    pub(crate) scene_view: wgpu::TextureView,
    // Read-only copy of scene for post passes that sample while writing to SceneColor
    pub(crate) scene_read: wgpu::Texture,
    pub(crate) scene_read_view: wgpu::TextureView,

    // Lighting M1: G-Buffer + Hi-Z scaffolding
    pub(crate) gbuffer: Option<crate::gfx::gbuffer::GBuffer>,
    pub(crate) hiz: Option<crate::gfx::hiz::HiZPyramid>,

    // --- Pipelines & BGLs ---
    pub(crate) pipeline: wgpu::RenderPipeline,
    pub(crate) inst_pipeline: wgpu::RenderPipeline,
    pub(crate) wire_pipeline: Option<wgpu::RenderPipeline>,
    pub(crate) particle_pipeline: wgpu::RenderPipeline,
    pub(crate) sky_pipeline: wgpu::RenderPipeline,
    pub(crate) post_ao_pipeline: wgpu::RenderPipeline,
    pub(crate) ssgi_pipeline: wgpu::RenderPipeline,
    pub(crate) ssr_pipeline: wgpu::RenderPipeline,
    pub(crate) present_pipeline: wgpu::RenderPipeline,
    pub(crate) blit_scene_read_pipeline: wgpu::RenderPipeline,
    pub(crate) bloom_pipeline: wgpu::RenderPipeline,
    pub(crate) bloom_bg: wgpu::BindGroup,
    pub(crate) direct_present: bool,
    pub(crate) lights_buf: wgpu::Buffer,
    // Stored bind group layouts needed to rebuild views on resize
    pub(crate) present_bgl: wgpu::BindGroupLayout,
    pub(crate) post_ao_bgl: wgpu::BindGroupLayout,
    pub(crate) ssgi_globals_bgl: wgpu::BindGroupLayout,
    pub(crate) ssgi_depth_bgl: wgpu::BindGroupLayout,
    pub(crate) ssgi_scene_bgl: wgpu::BindGroupLayout,
    pub(crate) ssr_depth_bgl: wgpu::BindGroupLayout,
    pub(crate) ssr_scene_bgl: wgpu::BindGroupLayout,
    pub(crate) palettes_bgl: wgpu::BindGroupLayout,
    pub(crate) globals_bg: wgpu::BindGroup,
    pub(crate) post_ao_bg: wgpu::BindGroup,
    pub(crate) ssgi_globals_bg: wgpu::BindGroup,
    pub(crate) ssgi_depth_bg: wgpu::BindGroup,
    pub(crate) ssgi_scene_bg: wgpu::BindGroup,
    pub(crate) ssr_depth_bg: wgpu::BindGroup,
    pub(crate) ssr_scene_bg: wgpu::BindGroup,
    pub(crate) _post_sampler: wgpu::Sampler,
    pub(crate) point_sampler: wgpu::Sampler,
    pub(crate) sky_bg: wgpu::BindGroup,
    pub(crate) terrain_model_bg: wgpu::BindGroup,
    pub(crate) shard_model_bg: wgpu::BindGroup,
    pub(crate) present_bg: wgpu::BindGroup,

    // Lighting toggles
    pub(crate) enable_post_ao: bool,
    pub(crate) enable_ssgi: bool,
    pub(crate) enable_ssr: bool,
    pub(crate) enable_bloom: bool,
    pub(crate) static_index: Option<collision_static::StaticIndex>,
    pub(crate) frame_counter: u32,
    // Stats
    pub(crate) draw_calls: u32,

    // --- Scene Buffers ---
    pub(crate) globals_buf: wgpu::Buffer,
    pub(crate) sky_buf: wgpu::Buffer,
    pub(crate) _plane_model_buf: wgpu::Buffer,
    pub(crate) shard_model_buf: wgpu::Buffer,

    // Geometry (terrain)
    pub(crate) terrain_vb: wgpu::Buffer,
    pub(crate) terrain_ib: wgpu::Buffer,
    pub(crate) terrain_index_count: u32,

    // GLTF geometry (wizard + ruins)
    pub(crate) wizard_vb: wgpu::Buffer,
    pub(crate) wizard_ib: wgpu::Buffer,
    pub(crate) wizard_index_count: u32,
    // Zombie skinned geometry
    pub(crate) zombie_vb: wgpu::Buffer,
    pub(crate) zombie_ib: wgpu::Buffer,
    pub(crate) zombie_index_count: u32,
    pub(crate) ruins_vb: wgpu::Buffer,
    pub(crate) ruins_ib: wgpu::Buffer,
    pub(crate) ruins_index_count: u32,

    // NPC cubes
    pub(crate) npc_vb: wgpu::Buffer,
    pub(crate) npc_ib: wgpu::Buffer,
    pub(crate) npc_index_count: u32,
    pub(crate) npc_instances: wgpu::Buffer,
    pub(crate) npc_count: u32,
    pub(crate) npc_instances_cpu: Vec<crate::gfx::types::Instance>,
    pub(crate) npc_models: Vec<glam::Mat4>,

    // Vegetation (trees) — instanced cubes for now
    pub(crate) trees_instances: wgpu::Buffer,
    pub(crate) trees_count: u32,
    pub(crate) trees_vb: wgpu::Buffer,
    pub(crate) trees_ib: wgpu::Buffer,
    pub(crate) trees_index_count: u32,

    // Rocks (instanced static mesh)
    pub(crate) rocks_instances: wgpu::Buffer,
    pub(crate) rocks_count: u32,
    pub(crate) rocks_vb: wgpu::Buffer,
    pub(crate) rocks_ib: wgpu::Buffer,
    pub(crate) rocks_index_count: u32,

    // Instancing buffers
    pub(crate) wizard_instances: wgpu::Buffer,
    pub(crate) wizard_count: u32,
    pub(crate) zombie_instances: wgpu::Buffer,
    pub(crate) zombie_count: u32,
    pub(crate) zombie_instances_cpu: Vec<crate::gfx::types::InstanceSkin>,
    pub(crate) ruins_instances: wgpu::Buffer,
    pub(crate) ruins_count: u32,

    // FX buffers
    pub(crate) fx_instances: wgpu::Buffer,
    pub(crate) _fx_capacity: u32,
    pub(crate) fx_count: u32,
    pub(crate) _fx_model_bg: wgpu::BindGroup,
    pub(crate) quad_vb: wgpu::Buffer,

    // Wizard skinning palettes
    pub(crate) palettes_buf: wgpu::Buffer,
    pub(crate) palettes_bg: wgpu::BindGroup,
    pub(crate) joints_per_wizard: u32,
    pub(crate) wizard_models: Vec<glam::Mat4>,
    pub(crate) wizard_instances_cpu: Vec<crate::gfx::types::InstanceSkin>,
    // Zombies
    pub(crate) zombie_palettes_buf: wgpu::Buffer,
    pub(crate) zombie_palettes_bg: wgpu::BindGroup,
    pub(crate) zombie_joints: u32,
    pub(crate) zombie_models: Vec<glam::Mat4>,
    pub(crate) zombie_cpu: ra_assets::types::SkinnedMeshCPU,
    pub(crate) zombie_time_offset: Vec<f32>,
    pub(crate) zombie_ids: Vec<server_core::NpcId>,
    pub(crate) zombie_prev_pos: Vec<glam::Vec3>,
    // Per-instance forward-axis offsets (authoring → world). Calibrated on movement.
    pub(crate) zombie_forward_offsets: Vec<f32>,

    // Wizard pipelines
    pub(crate) wizard_pipeline: wgpu::RenderPipeline,

    pub(crate) wizard_mat_bg: wgpu::BindGroup,
    pub(crate) _wizard_mat_buf: wgpu::Buffer,
    pub(crate) _wizard_tex_view: wgpu::TextureView,
    pub(crate) _wizard_sampler: wgpu::Sampler,
    pub(crate) zombie_mat_bg: wgpu::BindGroup,
    pub(crate) _zombie_mat_buf: wgpu::Buffer,
    pub(crate) _zombie_tex_view: wgpu::TextureView,
    pub(crate) _zombie_sampler: wgpu::Sampler,

    // Flags
    pub(crate) wire_enabled: bool,

    // Sky/time-of-day state
    pub(crate) sky: crate::gfx::sky::SkyStateCPU,

    // Terrain sampler (CPU)
    pub(crate) terrain_cpu: crate::gfx::terrain::TerrainCPU,

    // Time base for animation
    pub(crate) start: std::time::Instant,
    pub(crate) last_time: f32,

    // Wizard animation selection and time offsets
    pub(crate) wizard_anim_index: Vec<usize>,
    pub(crate) wizard_time_offset: Vec<f32>,

    // CPU-side skinned mesh data
    pub(crate) skinned_cpu: ra_assets::types::SkinnedMeshCPU,

    // Animation-driven VFX
    pub(crate) wizard_last_phase: Vec<f32>,
    pub(crate) hand_right_node: Option<usize>,
    pub(crate) root_node: Option<usize>,

    // Projectile + particle pools
    pub(crate) projectiles: Vec<crate::gfx::fx::Projectile>,
    pub(crate) particles: Vec<crate::gfx::fx::Particle>,

    // Data-driven spec
    pub(crate) fire_bolt: Option<data_runtime::spell::SpellSpec>,

    // UI overlay
    pub(crate) nameplates: crate::gfx::ui::Nameplates,
    pub(crate) nameplates_npc: crate::gfx::ui::Nameplates,
    pub(crate) bars: crate::gfx::ui::HealthBars,
    pub(crate) damage: crate::gfx::ui::DamageFloaters,
    pub(crate) hud: crate::gfx::ui::Hud,
    pub(crate) hud_model: ux_hud::HudModel,

    // --- Player/Camera ---
    pub(crate) pc_index: usize,
    pub(crate) player: client_core::controller::PlayerController,
    pub(crate) input: client_core::input::InputState,
    pub(crate) cam_follow: crate::gfx::camera_sys::FollowState,
    pub(crate) pc_cast_queued: bool,
    pub(crate) pc_cast_kind: Option<PcCast>,
    pub(crate) pc_anim_start: Option<f32>,
    pub(crate) pc_cast_time: f32,
    pub(crate) pc_cast_fired: bool,
    // Simple Fire Bolt cooldown tracking (seconds)
    pub(crate) firebolt_cd_until: f32,
    pub(crate) firebolt_cd_dur: f32,
    // Deprecated GCD tracking (not used when cast-time only)
    pub(crate) gcd_until: f32,
    pub(crate) gcd_duration: f32,
    // Orbit params
    pub(crate) cam_orbit_yaw: f32,
    pub(crate) cam_orbit_pitch: f32,
    pub(crate) cam_distance: f32,
    pub(crate) cam_lift: f32,
    pub(crate) cam_look_height: f32,
    pub(crate) rmb_down: bool,
    pub(crate) last_cursor_pos: Option<(f64, f64)>,

    // UI capture helpers
    pub(crate) screenshot_start: Option<f32>,

    // Server state (NPCs/health)
    pub(crate) server: server_core::ServerState,

    // Wizard health (including PC at pc_index)
    pub(crate) wizard_hp: Vec<i32>,
    pub(crate) wizard_hp_max: i32,
    pub(crate) pc_alive: bool,
}

