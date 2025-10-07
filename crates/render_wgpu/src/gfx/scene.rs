//! Demo scene assembly: spawns a small world and builds instance buffers.
//!
//! This module is intentionally simple and deterministic. It prepares instance
//! data for wizards (skinned) and ruins (static), assigns palette bases, and
//! returns a camera focus point to orbit.

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::gfx::terrain::TerrainCPU;
use crate::gfx::types::{Instance, InstanceSkin};
use ecs_core::{RenderKind, Transform, World};
#[cfg(feature = "vox_onepath_demo")]
use ra_assets::gltf::load_gltf_mesh;
use ra_assets::types::SkinnedMeshCPU;
use wgpu::util::DeviceExt;

pub struct SceneBuild {
    #[allow(dead_code)]
    pub wizard_instances: wgpu::Buffer,
    pub wizard_count: u32,
    pub ruins_instances: wgpu::Buffer,
    pub ruins_count: u32,
    pub ruins_instances_cpu: Vec<super::types::Instance>,
    pub joints_per_wizard: u32,
    pub wizard_anim_index: Vec<usize>,
    pub wizard_time_offset: Vec<f32>,
    pub cam_target: glam::Vec3,
    pub wizard_models: Vec<glam::Mat4>,
    /// CPU copy of instance data for wizards so we can update transforms per-frame.
    pub wizard_instances_cpu: Vec<InstanceSkin>,
    /// Index of the player character (PC) among wizards; others are NPCs.
    pub pc_index: usize,
    // Generic destructibles registry
    pub destruct_meshes_cpu: Vec<crate::gfx::DestructMeshCpu>,
    pub destruct_instances: Vec<crate::gfx::DestructInstance>,
}

pub fn build_demo_scene(
    device: &wgpu::Device,
    skinned_cpu: &SkinnedMeshCPU,
    plane_extent: f32,
    terrain: Option<&TerrainCPU>,
    ruins_base_offset: f32,
    ruins_radius: f32,
) -> SceneBuild {
    // Build a tiny ECS world and spawn entities
    let mut world = World::new();
    let mut rng = ChaCha8Rng::seed_from_u64(42);

    // Cluster wizards around a central one so the camera can see all of them.
    // Use one central PC and a single outward-facing ring of NPC wizards.
    let ring_count = 19usize; // number of NPC wizards on the outer ring
    // Center spawn; project onto terrain if available
    let mut center = glam::vec3(0.0, 0.0, 0.0);
    if let Some(t) = terrain {
        let (h, _n) = crate::gfx::terrain::height_at(t, center.x, center.z);
        center.y = h;
    }
    // Spawn the central wizard first (becomes camera target)
    world.spawn(
        Transform {
            translation: center,
            rotation: glam::Quat::IDENTITY,
            scale: glam::Vec3::splat(1.0),
        },
        RenderKind::Wizard,
    );
    // Inner ring removed (except center PC). We keep a single large ring below.
    // Place a set of ruins around the wizard circle
    // Ruins placement policy: by default, keep ruins in the distance so they
    // frame the scene without cluttering gameplay space. On desktop you can
    // opt into a few closer showcase pieces with `RA_RUINS_NEAR=1`.
    let ruins_near = std::env::var("RA_RUINS_NEAR")
        .map(|v| v == "1")
        .unwrap_or(false);
    let place_range = if ruins_near { 25.0 } else { plane_extent * 0.9 };
    // A few backdrop ruins placed far away for depth
    // The ruins model origin is roughly centered; raise Y so it rests on ground.
    let ruins_y = ruins_base_offset; // base offset aligns model min Y to ground with small embed
    if ruins_near {
        let ruins_positions = [
            glam::vec3(-place_range * 0.9, ruins_y, -place_range * 0.7),
            glam::vec3(place_range * 0.85, ruins_y, -place_range * 0.2),
            glam::vec3(-place_range * 0.2, ruins_y, place_range * 0.95),
        ];
        for pos in ruins_positions {
            let mut p = pos;
            let mut tilt = glam::Quat::IDENTITY;
            if let Some(t) = terrain {
                let (h, n) = height_min_under(t, p.x, p.z, ruins_radius);
                p.y = h + ruins_y;
                tilt = tilt_toward_normal(n, 8.0_f32.to_radians());
            }
            let yaw = glam::Quat::from_rotation_y(rng.random::<f32>() * std::f32::consts::TAU);
            let rotation = tilt * yaw;
            world.spawn(
                Transform {
                    translation: p,
                    rotation,
                    scale: glam::Vec3::splat(1.0),
                },
                RenderKind::Ruins,
            );
        }
    }
    // Additional distant ruins distributed on a wide ring for background depth
    // Fewer distant pieces keep the silhouette without overwhelming the scene.
    let far_count = 6usize;
    for i in 0..far_count {
        let base_a = (i as f32) / (far_count as f32) * std::f32::consts::TAU;
        let a = base_a + rng.random::<f32>() * 0.2 - 0.1; // jitter
        let r = place_range * (0.78 + rng.random::<f32>() * 0.15);
        let mut pos = glam::vec3(r * a.cos(), ruins_y, r * a.sin());
        let mut tilt = glam::Quat::IDENTITY;
        if let Some(t) = terrain {
            let (h, n) = height_min_under(t, pos.x, pos.z, ruins_radius);
            pos.y = h + ruins_y;
            tilt = tilt_toward_normal(n, 8.0_f32.to_radians());
        }
        let rot = tilt * glam::Quat::from_rotation_y(rng.random::<f32>() * std::f32::consts::TAU);
        world.spawn(
            Transform {
                translation: pos,
                rotation: rot,
                scale: glam::Vec3::splat(1.0),
            },
            RenderKind::Ruins,
        );
    }

    // Add one outward-facing ring of wizards
    let outer_ring_radius = 7.5f32; // wider circle for better spacing
    for i in 0..ring_count {
        let theta = (i as f32) / (ring_count as f32) * std::f32::consts::TAU;
        let mut translation = glam::vec3(
            outer_ring_radius * theta.cos(),
            0.0,
            outer_ring_radius * theta.sin(),
        );
        if let Some(t) = terrain {
            let (h, _n) = crate::gfx::terrain::height_at(t, translation.x, translation.z);
            translation.y = h;
        }
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

    // Assign palette bases and animations: PC idle in Still; all ring wizards PortalOpen (staggered).
    let joints_per_wizard = skinned_cpu.joints_nodes.len() as u32;
    let mut wizard_anim_index: Vec<usize> = Vec::with_capacity(wiz_instances.len());
    let mut wizard_time_offset: Vec<f32> = Vec::with_capacity(wiz_instances.len());
    for (i, inst) in wiz_instances.iter_mut().enumerate() {
        inst.palette_base = (i as u32) * joints_per_wizard;
        if i == 0 {
            // Center wizard (PC): idle in Still until casting
            wizard_anim_index.push(1);
            wizard_time_offset.push(0.0);
        } else {
            // Single ring: PortalOpen for all NPC wizards, staggered by 0.5s
            wizard_anim_index.push(0);
            let ring_idx = i - 1;
            wizard_time_offset.push(ring_idx as f32 * 0.5);
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
    log::debug!(
        "spawned {} wizards and {} ruins",
        wiz_instances.len(),
        ruin_instances.len()
    );

    SceneBuild {
        wizard_instances,
        wizard_count: wiz_instances.len() as u32,
        ruins_instances,
        ruins_count: ruin_instances.len() as u32,
        ruins_instances_cpu: ruin_instances.clone(),
        joints_per_wizard,
        wizard_anim_index,
        wizard_time_offset,
        cam_target,
        wizard_models,
        wizard_instances_cpu: wiz_instances,
        pc_index: 0,
        // Seed destructible registry on client only in legacy/demo builds.
        destruct_meshes_cpu: {
            #[cfg(not(feature = "vox_onepath_demo"))]
            {
                Vec::new()
            }
            #[cfg(feature = "vox_onepath_demo")]
            {
                // Load ruins mesh CPU once
                let path = crate::gfx::asset_path("assets/models/ruins.gltf");
                let mut v = Vec::new();
                if let Ok(cpu) = load_gltf_mesh(&path) {
                    // local AABB
                    let mut lmin = glam::Vec3::splat(f32::INFINITY);
                    let mut lmax = glam::Vec3::splat(f32::NEG_INFINITY);
                    for vert in &cpu.vertices {
                        let p = glam::Vec3::from(vert.pos);
                        lmin = lmin.min(p);
                        lmax = lmax.max(p);
                    }
                    v.push(crate::gfx::DestructMeshCpu {
                        positions: cpu.vertices.iter().map(|vv| vv.pos).collect(),
                        indices: cpu.indices.iter().map(|&i| i as u32).collect(),
                        local_min: [lmin.x, lmin.y, lmin.z],
                        local_max: [lmax.x, lmax.y, lmax.z],
                    });
                } else {
                    // Mesh load failed; keep empty and let runtime fall back to AABB proxy
                }
                v
            }
        },
        destruct_instances: {
            #[cfg(not(feature = "vox_onepath_demo"))]
            {
                Vec::new()
            }
            #[cfg(feature = "vox_onepath_demo")]
            {
                let mut insts = Vec::new();
                if !ruin_instances.clone().is_empty() {
                    // Compute world-space AABB per ruins instance
                    // For world AABB we can derive from local_min/max if mesh present; else set a small box
                    let (lm, l_max) = if let Some(dm) =
                        // shadow borrow ends at if scope
                        {
                            // hack: re-load path to fetch same local AABB; safe as tiny cost in builder
                            let path = crate::gfx::asset_path("assets/models/ruins.gltf");
                            if let Ok(cpu) = load_gltf_mesh(&path) {
                                let mut lmin = glam::Vec3::splat(f32::INFINITY);
                                let mut lmax = glam::Vec3::splat(f32::NEG_INFINITY);
                                for vert in &cpu.vertices {
                                    let p = glam::Vec3::from(vert.pos);
                                    lmin = lmin.min(p);
                                    lmax = lmax.max(p);
                                }
                                Some(([lmin.x, lmin.y, lmin.z], [lmax.x, lmax.y, lmax.z]))
                            } else {
                                None
                            }
                        } {
                        (glam::Vec3::from(dm.0), glam::Vec3::from(dm.1))
                    } else {
                        (glam::vec3(-3.0, -0.2, -3.0), glam::vec3(3.0, 2.8, 3.0))
                    };
                    for (i, inst) in ruin_instances.iter().enumerate() {
                        let model = glam::Mat4::from_cols_array_2d(&inst.model);
                        let corners = [
                            glam::vec3(lm.x, lm.y, lm.z),
                            glam::vec3(l_max.x, lm.y, lm.z),
                            glam::vec3(lm.x, l_max.y, lm.z),
                            glam::vec3(l_max.x, l_max.y, lm.z),
                            glam::vec3(lm.x, lm.y, l_max.z),
                            glam::vec3(l_max.x, lm.y, l_max.z),
                            glam::vec3(lm.x, l_max.y, l_max.z),
                            glam::vec3(l_max.x, l_max.y, l_max.z),
                        ];
                        let mut wmin = glam::Vec3::splat(f32::INFINITY);
                        let mut wmax = glam::Vec3::splat(f32::NEG_INFINITY);
                        for c in corners.iter() {
                            let wc = model.transform_point3(*c);
                            wmin = wmin.min(wc);
                            wmax = wmax.max(wc);
                        }
                        insts.push(crate::gfx::DestructInstance {
                            mesh_id: 0,
                            model,
                            source: crate::gfx::DestructSource::Ruins(i),
                            world_min: [wmin.x, wmin.y, wmin.z],
                            world_max: [wmax.x, wmax.y, wmax.z],
                        });
                    }
                }
                insts
            }
        },
    }
}

fn tilt_toward_normal(n: glam::Vec3, max_angle: f32) -> glam::Quat {
    let up = glam::Vec3::Y;
    let nn = n.normalize_or_zero();
    let dot = up.dot(nn).clamp(-1.0, 1.0);
    let full = dot.acos();
    let angle = full.min(max_angle);
    let axis = up.cross(nn);
    if axis.length_squared() < 1e-6 || angle < 1e-4 {
        glam::Quat::IDENTITY
    } else {
        glam::Quat::from_axis_angle(axis.normalize(), angle)
    }
}

fn height_min_under(t: &TerrainCPU, x: f32, z: f32, radius: f32) -> (f32, glam::Vec3) {
    // Sample center + four cardinal points at given radius; choose min height.
    let mut hmin = f32::INFINITY;
    let mut n_at = glam::Vec3::Y;
    let samples = [
        (0.0, 0.0),
        (radius, 0.0),
        (-radius, 0.0),
        (0.0, radius),
        (0.0, -radius),
    ];
    for (dx, dz) in samples {
        let (h, n) = crate::gfx::terrain::height_at(t, x + dx, z + dz);
        if h < hmin {
            hmin = h;
            n_at = n;
        }
    }
    (hmin, n_at)
}
