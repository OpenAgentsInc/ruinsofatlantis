//! Demo scene assembly: spawns a small world and builds instance buffers.
//!
//! This module is intentionally simple and deterministic. It prepares instance
//! data for wizards (skinned) and ruins (static), assigns palette bases, and
//! returns a camera focus point to orbit.

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::assets::SkinnedMeshCPU;
use crate::ecs::{RenderKind, Transform, World};
use crate::gfx::types::{Instance, InstanceSkin};
use wgpu::util::DeviceExt;

pub struct SceneBuild {
    pub wizard_instances: wgpu::Buffer,
    pub wizard_count: u32,
    pub ruins_instances: wgpu::Buffer,
    pub ruins_count: u32,
    pub joints_per_wizard: u32,
    pub wizard_anim_index: Vec<usize>,
    pub wizard_time_offset: Vec<f32>,
    pub cam_target: glam::Vec3,
    pub wizard_models: Vec<glam::Mat4>,
    /// CPU copy of instance data for wizards so we can update transforms per-frame.
    pub wizard_instances_cpu: Vec<InstanceSkin>,
    /// Index of the player character (PC) among wizards; others are NPCs.
    pub pc_index: usize,
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
        Transform {
            translation: center,
            rotation: glam::Quat::IDENTITY,
            scale: glam::Vec3::splat(1.0),
        },
        RenderKind::Wizard,
    );
    // Place remaining wizards on a small ring facing outward (away from the center)
    let ring_radius = 3.5f32;
    for i in 1..wizard_count {
        let theta = (i as f32 - 1.0) / (wizard_count as f32 - 1.0) * std::f32::consts::TAU;
        let translation = glam::vec3(ring_radius * theta.cos(), 0.0, ring_radius * theta.sin());
        // Face outward: yaw aligns +Z with (translation - center)
        let dx = translation.x - center.x;
        let dz = translation.z - center.z;
        let yaw = dx.atan2(dz);
        let rotation = glam::Quat::from_rotation_y(yaw);
        world.spawn(
            Transform {
                translation,
                rotation,
                scale: glam::Vec3::splat(1.0),
            },
            RenderKind::Wizard,
        );
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
        world.spawn(
            Transform {
                translation: pos,
                rotation,
                scale: glam::Vec3::splat(1.0),
            },
            RenderKind::Ruins,
        );
    }
    // Additional distant ruins distributed on a wide ring for background depth
    let far_count = 8usize;
    for i in 0..far_count {
        let base_a = (i as f32) / (far_count as f32) * std::f32::consts::TAU;
        let a = base_a + rng.random::<f32>() * 0.2 - 0.1; // jitter
        let r = place_range * (0.78 + rng.random::<f32>() * 0.15);
        let pos = glam::vec3(r * a.cos(), 0.0, r * a.sin());
        let rot = glam::Quat::from_rotation_y(rng.random::<f32>() * std::f32::consts::TAU);
        world.spawn(
            Transform {
                translation: pos,
                rotation: rot,
                scale: glam::Vec3::splat(1.0),
            },
            RenderKind::Ruins,
        );
    }

    // Add an outer ring of wizards facing outward
    let outer_ring_radius = ring_radius * 2.1;
    let outer_count = wizard_count; // same count as inner ring
    for i in 0..outer_count {
        let theta = (i as f32) / (outer_count as f32) * std::f32::consts::TAU;
        let translation = glam::vec3(
            outer_ring_radius * theta.cos(),
            0.0,
            outer_ring_radius * theta.sin(),
        );
        // Face outward: yaw aligns +Z with (translation - center)
        let dx = translation.x - center.x;
        let dz = translation.z - center.z;
        let yaw = dx.atan2(dz);
        let rotation = glam::Quat::from_rotation_y(yaw);
        world.spawn(
            Transform {
                translation,
                rotation,
                scale: glam::Vec3::splat(1.0),
            },
            RenderKind::Wizard,
        );
    }

    // Build instance lists
    let mut wiz_instances: Vec<InstanceSkin> = Vec::new();
    let mut wizard_models: Vec<glam::Mat4> = Vec::new();
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
                wizard_models.push(glam::Mat4::from_cols_array_2d(&m));
                wiz_instances.push(InstanceSkin {
                    model: m,
                    color: [0.20, 0.45, 0.95],
                    // Mark center wizard (first) as selected (PC)
                    selected: if wizard_models.len() == 1 { 1.0 } else { 0.0 },
                    palette_base: 0,
                    _pad_inst: [0; 3],
                })
            }
            RenderKind::Ruins => ruin_instances.push(Instance {
                model: m,
                color: [0.65, 0.66, 0.68],
                selected: 0.0,
            }),
        }
    }

    // Assign palette bases and animations: inner ring faces inward (center uses PortalOpen,
    // others Waiting); outer ring faces outward (all PortalOpen). Stagger PortalOpen starts by 0.5s.
    let joints_per_wizard = skinned_cpu.joints_nodes.len() as u32;
    let mut wizard_anim_index: Vec<usize> = Vec::with_capacity(wiz_instances.len());
    let mut wizard_time_offset: Vec<f32> = Vec::with_capacity(wiz_instances.len());
    for (i, inst) in wiz_instances.iter_mut().enumerate() {
        inst.palette_base = (i as u32) * joints_per_wizard;
        if i == 0 {
            // Center wizard (PC): idle in Still until casting
            wizard_anim_index.push(1);
            wizard_time_offset.push(0.0);
        } else if i < wizard_count {
            // Inner ring (excluding center): Still only
            wizard_anim_index.push(1);
            wizard_time_offset.push(0.0);
        } else {
            // Outer ring: PortalOpen, staggered by 0.5s
            wizard_anim_index.push(0);
            let outer_idx = i - wizard_count;
            wizard_time_offset.push(outer_idx as f32 * 0.5);
        }
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
    log::info!(
        "spawned {} wizards and {} ruins",
        wiz_instances.len(),
        ruin_instances.len()
    );

    SceneBuild {
        wizard_instances,
        wizard_count: wiz_instances.len() as u32,
        ruins_instances,
        ruins_count: ruin_instances.len() as u32,
        joints_per_wizard,
        wizard_anim_index,
        wizard_time_offset,
        cam_target,
        wizard_models,
        wizard_instances_cpu: wiz_instances,
        pc_index: 0,
    }
}
