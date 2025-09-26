//! Demo scene assembly: spawns a small world and builds instance buffers.
//!
//! This module is intentionally simple and deterministic. It prepares instance
//! data for wizards (skinned) and ruins (static), assigns palette bases, and
//! returns a camera focus point to orbit.

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::assets::SkinnedMeshCPU;
use wgpu::util::DeviceExt;
use crate::ecs::{RenderKind, Transform, World};
use crate::gfx::types::{Instance, InstanceSkin};

pub struct SceneBuild {
    pub wizard_instances: wgpu::Buffer,
    pub wizard_count: u32,
    pub ruins_instances: wgpu::Buffer,
    pub ruins_count: u32,
    pub joints_per_wizard: u32,
    pub wizard_anim_index: Vec<usize>,
    pub wizard_time_offset: Vec<f32>,
    pub cam_target: glam::Vec3,
}

pub fn build_demo_scene(
    device: &wgpu::Device,
    skinned_cpu: &SkinnedMeshCPU,
    plane_extent: f32,
) -> SceneBuild {
    // Build a tiny ECS world and spawn entities
    let mut world = World::new();
    let mut rng = ChaCha8Rng::seed_from_u64(42);

    // Cluster wizards around a central one so the camera can see all of them.
    let wizard_count = 10usize;
    let center = glam::vec3(0.0, 0.0, 0.0);
    // Spawn the central wizard first (becomes camera target)
    world.spawn(
        Transform { translation: center, rotation: glam::Quat::IDENTITY, scale: glam::Vec3::splat(1.0) },
        RenderKind::Wizard,
    );
    // Place remaining wizards on a small ring facing the center
    let ring_radius = 3.5f32;
    for i in 1..wizard_count {
        let theta = (i as f32 - 1.0) / (wizard_count as f32 - 1.0) * std::f32::consts::TAU;
        let translation = glam::vec3(ring_radius * theta.cos(), 0.0, ring_radius * theta.sin());
        // Face the center with yaw that aligns +Z to (center - translation)
        let dx = center.x - translation.x;
        let dz = center.z - translation.z;
        let yaw = dx.atan2(dz);
        let rotation = glam::Quat::from_rotation_y(yaw);
        world.spawn(Transform { translation, rotation, scale: glam::Vec3::splat(1.0) }, RenderKind::Wizard);
    }
    // Place a set of ruins around the wizard circle
    let place_range = plane_extent * 0.9;
    // A few backdrop ruins placed far away for depth
    let ruins_positions = [
        glam::vec3(-place_range * 0.9, 0.0, -place_range * 0.7),
        glam::vec3(place_range * 0.85, 0.0, -place_range * 0.2),
        glam::vec3(-place_range * 0.2, 0.0, place_range * 0.95),
    ];
    for pos in ruins_positions {
        let rotation = glam::Quat::from_rotation_y(rng.random::<f32>() * std::f32::consts::TAU);
        world.spawn(Transform { translation: pos, rotation, scale: glam::Vec3::splat(1.0) }, RenderKind::Ruins);
    }
    // Additional distant ruins distributed on a wide ring for background depth
    let far_count = 8usize;
    for i in 0..far_count {
        let base_a = (i as f32) / (far_count as f32) * std::f32::consts::TAU;
        let a = base_a + rng.random::<f32>() * 0.2 - 0.1; // jitter
        let r = place_range * (0.78 + rng.random::<f32>() * 0.15);
        let pos = glam::vec3(r * a.cos(), 0.0, r * a.sin());
        let rot = glam::Quat::from_rotation_y(rng.random::<f32>() * std::f32::consts::TAU);
        world.spawn(Transform { translation: pos, rotation: rot, scale: glam::Vec3::splat(1.0) }, RenderKind::Ruins);
    }

    // Build instance lists
    let mut wiz_instances: Vec<InstanceSkin> = Vec::new();
    let mut ruin_instances: Vec<Instance> = Vec::new();
    let mut cam_target = glam::Vec3::ZERO;
    let mut has_cam_target = false;
    for (i, kind) in world.kinds.iter().enumerate() {
        let t = world.transforms[i];
        let m = t.matrix().to_cols_array_2d();
        match kind {
            RenderKind::Wizard => {
                if !has_cam_target {
                    cam_target = t.translation + glam::vec3(0.0, 1.2, 0.0);
                    has_cam_target = true;
                }
                wiz_instances.push(InstanceSkin { model: m, color: [0.20, 0.45, 0.95], selected: 0.0, palette_base: 0, _pad_inst: [0; 3] })
            }
            RenderKind::Ruins => ruin_instances.push(Instance { model: m, color: [0.65, 0.66, 0.68], selected: 0.0 }),
        }
    }

    // Assign palette bases and random animations
    let joints_per_wizard = skinned_cpu.joints_nodes.len() as u32;
    let mut rng2 = ChaCha8Rng::seed_from_u64(4242);
    let mut wizard_anim_index: Vec<usize> = Vec::with_capacity(wiz_instances.len());
    let mut wizard_time_offset: Vec<f32> = Vec::with_capacity(wiz_instances.len());
    for (i, inst) in wiz_instances.iter_mut().enumerate() {
        inst.palette_base = (i as u32) * joints_per_wizard;
        if i == 0 { wizard_anim_index.push(0); } else { wizard_anim_index.push(2); }
        wizard_time_offset.push(rng2.random::<f32>() * 1.7);
    }

    let wizard_instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("wizard-instances"),
        contents: bytemuck::cast_slice(&wiz_instances),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    });
    let ruins_instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ruins-instances"),
        contents: bytemuck::cast_slice(&ruin_instances),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    });
    log::info!("spawned {} wizards and {} ruins", wiz_instances.len(), ruin_instances.len());

    SceneBuild {
        wizard_instances,
        wizard_count: wiz_instances.len() as u32,
        ruins_instances,
        ruins_count: ruin_instances.len() as u32,
        joints_per_wizard,
        wizard_anim_index,
        wizard_time_offset,
        cam_target,
    }
}
