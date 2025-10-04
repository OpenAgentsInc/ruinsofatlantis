//! CPU-side update helpers extracted from gfx/mod.rs

// use Debris via fully-qualified path
use crate::gfx::Renderer;
#[cfg(any(feature = "legacy_client_carve", feature = "vox_onepath_demo"))]
use crate::gfx::chunkcol;
use crate::gfx::types::{Instance, InstanceSkin, ParticleInstance};
use crate::gfx::{self, anim, fx::Particle, terrain};
use crate::server_ext::CollideProjectiles;
#[cfg(any(feature = "legacy_client_carve", feature = "vox_onepath_demo"))]
use glam::DVec3;
use ra_assets::types::AnimClip;
use rand::Rng as _;
// use destructible via fully-qualified path
#[cfg(any(feature = "legacy_client_carve", feature = "vox_onepath_demo"))]
use server_core::destructible::{carve_and_spawn_debris, raycast_voxels};
#[cfg(all(
    any(feature = "legacy_client_carve", feature = "vox_onepath_demo"),
    not(target_arch = "wasm32")
))]
use std::time::Instant;
#[cfg(any(feature = "legacy_client_carve", feature = "vox_onepath_demo"))]
use voxel_proxy::{VoxelProxyMeta, voxelize_surface_fill};
#[cfg(all(
    any(feature = "legacy_client_carve", feature = "vox_onepath_demo"),
    target_arch = "wasm32"
))]
use web_time::Instant;
// device buffer init now handled via voxel_upload helper

// Opt-in logging for destructible workflows
#[macro_export]
macro_rules! destruct_log {
    ($($tt:tt)*) => {
        #[cfg(feature = "destruct_debug")]
        log::info!($($tt)*);
    };
}

// Tiny deterministic RNG for demo variations (no external deps)
#[inline]
#[cfg_attr(
    not(any(feature = "legacy_client_carve", feature = "vox_onepath_demo")),
    allow(dead_code)
)]
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[inline]
#[cfg_attr(
    not(any(feature = "legacy_client_carve", feature = "vox_onepath_demo")),
    allow(dead_code)
)]
fn rand01(s: &mut u64) -> f32 {
    let r = splitmix64(s);
    ((r >> 40) as u32) as f32 / (1u32 << 24) as f32
}

#[inline]
#[cfg_attr(
    not(any(feature = "legacy_client_carve", feature = "vox_onepath_demo")),
    allow(dead_code)
)]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

impl Renderer {
    // Approx ruin local-space AABB for fast world AABB expansion
    const RUIN_LOCAL_MIN_X: f32 = -3.0;
    const RUIN_LOCAL_MIN_Y: f32 = -0.2;
    const RUIN_LOCAL_MIN_Z: f32 = -3.0;
    const RUIN_LOCAL_MAX_X: f32 = 3.0;
    const RUIN_LOCAL_MAX_Y: f32 = 2.8;
    const RUIN_LOCAL_MAX_Z: f32 = 3.0;
    // Small helper: intersect camera ray with a box
    #[inline]
    fn ray_box_intersect(
        p0: glam::Vec3,
        dir: glam::Vec3,
        bmin: glam::Vec3,
        bmax: glam::Vec3,
    ) -> Option<(f32, f32)> {
        let inv = glam::Vec3::new(
            if dir.x.abs() > 1e-8 {
                1.0 / dir.x
            } else {
                f32::INFINITY
            },
            if dir.y.abs() > 1e-8 {
                1.0 / dir.y
            } else {
                f32::INFINITY
            },
            if dir.z.abs() > 1e-8 {
                1.0 / dir.z
            } else {
                f32::INFINITY
            },
        );
        let t0s = (bmin - p0) * inv;
        let t1s = (bmax - p0) * inv;
        let tmin = t0s.min(t1s);
        let tmax = t0s.max(t1s);
        let t_enter = tmin.x.max(tmin.y).max(tmin.z);
        let t_exit = tmax.x.min(tmax.y).min(tmax.z);
        if t_exit >= t_enter.max(0.0) {
            Some((t_enter.max(0.0), t_exit))
        } else {
            None
        }
    }

    #[inline]
    fn ruin_world_aabb(&self, idx: usize) -> (glam::Vec3, glam::Vec3) {
        let inst = &self.ruins_instances_cpu[idx];
        let m = glam::Mat4::from_cols_array_2d(&inst.model);
        let corners = [
            glam::vec3(
                Self::RUIN_LOCAL_MIN_X,
                Self::RUIN_LOCAL_MIN_Y,
                Self::RUIN_LOCAL_MIN_Z,
            ),
            glam::vec3(
                Self::RUIN_LOCAL_MIN_X,
                Self::RUIN_LOCAL_MIN_Y,
                Self::RUIN_LOCAL_MAX_Z,
            ),
            glam::vec3(
                Self::RUIN_LOCAL_MIN_X,
                Self::RUIN_LOCAL_MAX_Y,
                Self::RUIN_LOCAL_MIN_Z,
            ),
            glam::vec3(
                Self::RUIN_LOCAL_MIN_X,
                Self::RUIN_LOCAL_MAX_Y,
                Self::RUIN_LOCAL_MAX_Z,
            ),
            glam::vec3(
                Self::RUIN_LOCAL_MAX_X,
                Self::RUIN_LOCAL_MIN_Y,
                Self::RUIN_LOCAL_MIN_Z,
            ),
            glam::vec3(
                Self::RUIN_LOCAL_MAX_X,
                Self::RUIN_LOCAL_MIN_Y,
                Self::RUIN_LOCAL_MAX_Z,
            ),
            glam::vec3(
                Self::RUIN_LOCAL_MAX_X,
                Self::RUIN_LOCAL_MAX_Y,
                Self::RUIN_LOCAL_MIN_Z,
            ),
            glam::vec3(
                Self::RUIN_LOCAL_MAX_X,
                Self::RUIN_LOCAL_MAX_Y,
                Self::RUIN_LOCAL_MAX_Z,
            ),
        ];
        let mut bmin = glam::Vec3::splat(f32::INFINITY);
        let mut bmax = glam::Vec3::splat(f32::NEG_INFINITY);
        for c in &corners {
            let w = m.transform_point3(*c);
            bmin = bmin.min(w);
            bmax = bmax.max(w);
        }
        let mrg = 0.20f32;
        (bmin - glam::Vec3::splat(mrg), bmax + glam::Vec3::splat(mrg))
    }

    #[inline]
    #[allow(dead_code)]
    fn segment_hits_ruins(&self, p0: glam::Vec3, p1: glam::Vec3) -> Option<(usize, f32)> {
        if self.ruins_instances_cpu.is_empty() {
            return None;
        }
        let d = p1 - p0;
        let seg_len = d.length();
        if seg_len < 1e-6 {
            return None;
        }
        let dir = d / seg_len;
        let mut best = (usize::MAX, seg_len + 1.0);
        for i in 0..self.ruins_instances_cpu.len() {
            let (bmin, bmax) = self.ruin_world_aabb(i);
            if let Some((t_enter, _)) = Self::ray_box_intersect(p0, dir, bmin, bmax)
                && t_enter <= seg_len
                && t_enter < best.1
            {
                best = (i, t_enter);
            }
        }
        (best.0 != usize::MAX).then_some(best)
    }

    #[inline]
    fn grid_world_aabb(grid: &voxel_proxy::VoxelGrid) -> (glam::Vec3, glam::Vec3) {
        let o = grid.origin_m();
        let vm = grid.voxel_m().0 as f32;
        let d = grid.dims();
        let bmin = glam::vec3(o.x as f32, o.y as f32, o.z as f32);
        let bmax = bmin + glam::vec3(d.x as f32 * vm, d.y as f32 * vm, d.z as f32 * vm);
        (bmin, bmax)
    }

    #[inline]
    #[allow(dead_code)]
    fn segment_hits_any_proxy(&self, p0: glam::Vec3, p1: glam::Vec3) -> Option<(usize, f32)> {
        if self.destr_voxels.is_empty() {
            return None;
        }
        let d = p1 - p0;
        let seg_len = d.length();
        if seg_len < 1e-6 {
            return None;
        }
        // Segment vs AABB (padded) selection
        let mut hit: Option<(usize, f32)> = None;
        for (ri, rv) in &self.destr_voxels {
            let (bmin, bmax) = Self::grid_world_aabb(&rv.grid);
            let pad = 0.25f32;
            let bmin = bmin - glam::Vec3::splat(pad);
            let bmax = bmax + glam::Vec3::splat(pad);
            // slab test
            let mut tmin = 0.0f32;
            let mut tmax = 1.0f32;
            let mut ok = true;
            for i in 0..3 {
                let s = p0[i];
                let dirc = d[i];
                let minb = bmin[i];
                let maxb = bmax[i];
                if dirc.abs() < 1e-6 {
                    if s < minb || s > maxb {
                        ok = false;
                        break;
                    }
                } else {
                    let inv = 1.0 / dirc;
                    let mut t0 = (minb - s) * inv;
                    let mut t1 = (maxb - s) * inv;
                    if t0 > t1 {
                        core::mem::swap(&mut t0, &mut t1);
                    }
                    tmin = tmin.max(t0);
                    tmax = tmax.min(t1);
                    if tmin > tmax {
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                hit = Some((ri.0, tmin));
                break;
            }
        }
        hit
    }

    // Combined selector: proxies first (nearest), then any destructible instance (cached AABBs)
    fn find_destructible_hit(
        &self,
        p0: glam::Vec3,
        p1: glam::Vec3,
    ) -> Option<(crate::gfx::DestructibleId, f32)> {
        let pad = 0.25f32;
        // 1) proxies first
        let mut best: Option<(crate::gfx::DestructibleId, f32)> = None;
        let d = p1 - p0;
        for (did, vp) in &self.destr_voxels {
            let (bmin, bmax) = Self::grid_world_aabb(&vp.grid);
            let bmin = bmin - glam::Vec3::splat(pad);
            let bmax = bmax + glam::Vec3::splat(pad);
            let mut tmin = 0.0f32;
            let mut tmax = 1.0f32;
            let mut ok = true;
            for i in 0..3 {
                let s = p0[i];
                let dirc = d[i];
                let minb = bmin[i];
                let maxb = bmax[i];
                if dirc.abs() < 1e-6 {
                    if s < minb || s > maxb {
                        ok = false;
                        break;
                    }
                } else {
                    let inv = 1.0 / dirc;
                    let mut t0 = (minb - s) * inv;
                    let mut t1 = (maxb - s) * inv;
                    if t0 > t1 {
                        core::mem::swap(&mut t0, &mut t1);
                    }
                    tmin = tmin.max(t0);
                    tmax = tmax.min(t1);
                    if tmin > tmax {
                        ok = false;
                        break;
                    }
                }
            }
            if ok && best.is_none_or(|(_, tb)| tmin < tb) {
                best = Some((*did, tmin));
            }
        }
        if best.is_some() {
            if let Some((_did, _t)) = best {
                destruct_log!("[destruct] select(proxy) did={:?} t={:.3}", _did, _t);
            }
            return best;
        }
        // 2) instances fallback (skip any with an existing proxy)
        for (i, di) in self.destruct_instances.iter().enumerate() {
            let did = crate::gfx::DestructibleId(i);
            if self.destr_voxels.contains_key(&did) {
                continue;
            }
            let bmin = glam::Vec3::from(di.world_min) - glam::Vec3::splat(pad);
            let bmax = glam::Vec3::from(di.world_max) + glam::Vec3::splat(pad);
            let mut tmin = 0.0f32;
            let mut tmax = 1.0f32;
            let mut ok = true;
            for i in 0..3 {
                let s = p0[i];
                let dirc = d[i];
                let minb = bmin[i];
                let maxb = bmax[i];
                if dirc.abs() < 1e-6 {
                    if s < minb || s > maxb {
                        ok = false;
                        break;
                    }
                } else {
                    let inv = 1.0 / dirc;
                    let mut t0 = (minb - s) * inv;
                    let mut t1 = (maxb - s) * inv;
                    if t0 > t1 {
                        core::mem::swap(&mut t0, &mut t1);
                    }
                    tmin = tmin.max(t0);
                    tmax = tmax.min(t1);
                    if tmin > tmax {
                        ok = false;
                        break;
                    }
                }
            }
            if ok && best.is_none_or(|(_, tb)| tmin < tb) {
                best = Some((did, tmin));
            }
        }
        if let Some((_did, _t)) = best {
            destruct_log!("[destruct] select(instance) did={:?} t={:.3}", _did, _t);
        } else {
            destruct_log!("[destruct] select: no hit on proxies or instances");
        }
        best
    }

    #[cfg(feature = "legacy_client_carve")]
    fn get_or_spawn_proxy(&mut self, did: crate::gfx::DestructibleId) -> &mut crate::gfx::VoxProxy {
        // For now, reuse ruins-specific spawner; generic builder can be added later
        self.get_or_spawn_ruin_proxy(did.0)
    }

    #[cfg(feature = "legacy_client_carve")]
    #[allow(clippy::too_many_arguments)]
    fn explode_fireball_against_destructible(
        &mut self,
        owner: Option<usize>,
        p0: glam::Vec3,
        p1: glam::Vec3,
        did: crate::gfx::DestructibleId,
        t_hit: f32,
        radius: f32,
        damage: i32,
    ) {
        self.explode_fireball_at(owner, p1, radius, damage);
        let seed0 = self.destruct_cfg.seed;
        let impact0 = self.impact_id;
        let max_debris0 = self.destruct_cfg.max_debris;
        let vp = self.get_or_spawn_proxy(did);
        let dims = vp.grid.dims();
        destruct_log!(
            "[destruct] carve: did={:?} grid dims={}x{}x{} vm={:.3}",
            did,
            dims.x,
            dims.y,
            dims.z,
            vp.grid.voxel_m().0 as f32
        );
        let seg = p1 - p0;
        let len = seg.length();
        if len < 1e-6 {
            destruct_log!("[destruct] carve: zero-length segment; skipping");
            return;
        }
        let vm = vp.grid.voxel_m().0 as f32;
        // Use hit t from selector (computed vs padded AABB); step a tiny epsilon inward
        let eps_n = (vm * 1e-3) / len.max(1e-6);
        let p_entry = p0 + seg * (t_hit + eps_n);
        destruct_log!(
            "[destruct] carve: entry t={:.3} p_entry=({:.2},{:.2},{:.2})",
            t_hit,
            p_entry.x,
            p_entry.y,
            p_entry.z
        );
        // Give DDA enough distance to find first shell: radius + a few voxels
        let dda_max = radius.max(vm * 2.0) + vm * 8.0;
        if let Some(hit) = server_core::destructible::raycast_voxels(
            &vp.grid,
            glam::DVec3::new(p_entry.x as f64, p_entry.y as f64, p_entry.z as f64),
            glam::DVec3::new(seg.x as f64, seg.y as f64, seg.z as f64).normalize(),
            core_units::Length::meters(dda_max as f64),
        ) {
            destruct_log!(
                "[destruct] carve: DDA hit voxel=({}, {}, {})",
                hit.voxel.x,
                hit.voxel.y,
                hit.voxel.z
            );
            let vc = glam::DVec3::new(
                hit.voxel.x as f64 + 0.5,
                hit.voxel.y as f64 + 0.5,
                hit.voxel.z as f64 + 0.5,
            );
            let impact = vp.grid.origin_m() + vc * vp.grid.voxel_m().0;
            let out = {
                let g = &mut self.get_or_spawn_proxy(did).grid;
                server_core::destructible::carve_and_spawn_debris(
                    g,
                    impact,
                    core_units::Length::meters((radius * 0.25) as f64),
                    seed0 ^ impact0,
                    impact0,
                    max_debris0,
                )
            };
            self.impact_id = self.impact_id.wrapping_add(1);
            // Pop dirty chunks without holding a mutable borrow across updates
            let dirty = {
                let vp2 = self.get_or_spawn_proxy(did);
                vp2.grid.pop_dirty_chunks(usize::MAX)
            };
            if !dirty.is_empty() {
                destruct_log!(
                    "[destruct] carve: did={:?} enq dirty={} debris+{}",
                    did,
                    dirty.len(),
                    out.positions_m.len()
                );
                let updates: Vec<_> = {
                    let vp2 = self.get_or_spawn_proxy(did);
                    dirty
                        .iter()
                        .filter_map(|c| chunkcol::build_chunk_collider(&vp2.grid, *c))
                        .collect()
                };
                if !updates.is_empty() {
                    chunkcol::swap_in_updates(&mut self.chunk_colliders, updates);
                    let idx = chunkcol::rebuild_static_index(&self.chunk_colliders);
                    self.static_index = Some(idx);
                }
                let vp2 = self.get_or_spawn_proxy(did);
                vp2.chunk_queue.enqueue_many(dirty);
                vp2.queue_len = vp2.chunk_queue.len();
                destruct_log!(
                    "[destruct] queue: did={:?} len={} (post-enqueue)",
                    did,
                    vp2.queue_len
                );
            }
            for (i, p) in out.positions_m.iter().enumerate() {
                if (self.debris.len() as u32) >= self.debris_capacity {
                    break;
                }
                let pos = glam::vec3(p.x as f32, p.y as f32, p.z as f32);
                let vel = out
                    .velocities_mps
                    .get(i)
                    .map(|v| glam::vec3(v.x as f32, v.y as f32, v.z as f32))
                    .unwrap_or(glam::Vec3::Y * 2.5);
                self.debris.push(crate::gfx::Debris {
                    pos,
                    vel,
                    age: 0.0,
                    life: 2.5,
                });
            }
        } else {
            destruct_log!(
                "[destruct] carve: DDA found no solid (dda_max={:.2}m, vm={:.3}m)",
                dda_max,
                vm
            );
        }
        let before = self.voxel_meshes.len();
        #[cfg(any(feature = "legacy_client_carve", feature = "vox_onepath_demo"))]
        self.process_all_ruin_queues();
        let after = self.voxel_meshes.len();
        log::info!("[destruct] mesh upload: chunks {} → {}", before, after);
    }

    #[cfg(feature = "legacy_client_carve")]
    #[cfg(any(feature = "legacy_client_carve", feature = "vox_onepath_demo"))]
    fn build_ruin_proxy_from_aabb(
        &self,
        ruin_idx: usize,
        bmin: glam::Vec3,
        bmax: glam::Vec3,
    ) -> crate::gfx::RuinVox {
        let center = (bmin + bmax) * 0.5;
        let size = (bmax - bmin).max(glam::Vec3::splat(0.5));
        // Coarsen voxel size until cells <= ~400k
        let mut vm = self.destruct_cfg.voxel_size_m.0 as f32;
        const MAX_CELLS: u64 = 400_000;
        let dims = loop {
            let dx = ((size.x / vm).ceil().max(1.0) as u32).max(12);
            let dy = ((size.y / vm).ceil().max(1.0) as u32).max(12);
            let dz = ((size.z / vm).ceil().max(1.0) as u32).max(12);
            if (dx as u64) * (dy as u64) * (dz as u64) <= MAX_CELLS {
                break glam::UVec3::new(dx, dy, dz);
            }
            vm *= 1.25;
        };
        let origin = glam::DVec3::new(
            (center.x - 0.5 * dims.x as f32 * vm) as f64,
            (center.y - 0.5 * dims.y as f32 * vm) as f64,
            (center.z - 0.5 * dims.z as f32 * vm) as f64,
        );
        let meta = voxel_proxy::VoxelProxyMeta {
            object_id: voxel_proxy::GlobalId(1000u64 + ruin_idx as u64),
            origin_m: origin,
            voxel_m: core_units::Length::meters(vm as f64),
            dims,
            chunk: self.destruct_cfg.chunk.min(dims.max(glam::UVec3::splat(1))),
            material: self.destruct_cfg.material,
        };
        let mut grid = voxel_proxy::VoxelGrid::new(meta);
        for z in 0..dims.z {
            for y in 0..dims.y {
                for x in 0..dims.x {
                    grid.set(x, y, z, true);
                }
            }
        }
        // Colliders
        let csz = grid.meta().chunk;
        let nx = dims.x.div_ceil(csz.x);
        let ny = dims.y.div_ceil(csz.y);
        let nz = dims.z.div_ceil(csz.z);
        let mut colliders = Vec::new();
        for cz in 0..nz {
            for cy in 0..ny {
                for cx in 0..nx {
                    if let Some(sc) =
                        chunkcol::build_chunk_collider(&grid, glam::UVec3::new(cx, cy, cz))
                    {
                        colliders.push(sc);
                    }
                }
            }
        }
        let static_index = Some(chunkcol::rebuild_static_index(&colliders));
        log::info!(
            "[destruct] proxy(box) ruin={} dims={}x{}x{} vm={:.3}",
            ruin_idx,
            dims.x,
            dims.y,
            dims.z,
            vm
        );
        crate::gfx::RuinVox {
            grid,
            chunk_queue: server_core::destructible::queue::ChunkQueue::new(),
            queue_len: 0,
            colliders,
            static_index,
        }
    }

    // Build a proxy using the actual ruins mesh triangles transformed by the instance model
    #[cfg(feature = "legacy_client_carve")]
    #[cfg(any(feature = "legacy_client_carve", feature = "vox_onepath_demo"))]
    fn build_ruin_proxy_from_mesh(&self, ruin_idx: usize) -> crate::gfx::RuinVox {
        // Fetch CPU triangles for ruins; fallback to AABB solid box if unavailable
        if self.destruct_meshes_cpu.is_empty() {
            log::warn!(
                "[destruct] no CPU mesh; falling back to AABB proxy for ruin {}",
                ruin_idx
            );
            let (bmin, bmax) = self.ruin_world_aabb(ruin_idx);
            return self.build_ruin_proxy_from_aabb(ruin_idx, bmin, bmax);
        }
        let mesh = &self.destruct_meshes_cpu[0];
        let m = glam::Mat4::from_cols_array_2d(&self.ruins_instances_cpu[ruin_idx].model);
        // Transform positions to world
        let mut tris: Vec<[glam::Vec3; 3]> = Vec::with_capacity(mesh.indices.len() / 3);
        for tri in mesh.indices.chunks_exact(3) {
            let a = m.transform_point3(glam::Vec3::from(mesh.positions[tri[0] as usize]));
            let b = m.transform_point3(glam::Vec3::from(mesh.positions[tri[1] as usize]));
            let c = m.transform_point3(glam::Vec3::from(mesh.positions[tri[2] as usize]));
            tris.push([a, b, c]);
        }
        log::info!(
            "[destruct] proxy(mesh) ruin={} tris={}",
            ruin_idx,
            tris.len()
        );
        // World AABB from triangles
        let mut bmin = glam::Vec3::splat(f32::INFINITY);
        let mut bmax = glam::Vec3::splat(f32::NEG_INFINITY);
        for [a, b, c] in &tris {
            bmin = bmin.min(*a).min(*b).min(*c);
            bmax = bmax.max(*a).max(*b).max(*c);
        }
        let mrg = 0.10;
        bmin -= glam::Vec3::splat(mrg);
        bmax += glam::Vec3::splat(mrg);
        // Grid sizing with clamp
        let size = (bmax - bmin).max(glam::Vec3::splat(0.5));
        let mut vm = self.destruct_cfg.voxel_size_m.0 as f32;
        const MAX_CELLS: u64 = 400_000;
        let dims = loop {
            let dx = ((size.x / vm).ceil().max(1.0) as u32).max(12);
            let dy = ((size.y / vm).ceil().max(1.0) as u32).max(12);
            let dz = ((size.z / vm).ceil().max(1.0) as u32).max(12);
            if (dx as u64) * (dy as u64) * (dz as u64) <= MAX_CELLS {
                break glam::UVec3::new(dx, dy, dz);
            }
            vm *= 1.25;
        };
        let origin = glam::DVec3::new(bmin.x as f64, bmin.y as f64, bmin.z as f64);
        let meta = VoxelProxyMeta {
            object_id: voxel_proxy::GlobalId(1000u64 + ruin_idx as u64),
            origin_m: origin,
            voxel_m: core_units::Length::meters(vm as f64),
            dims,
            chunk: self.destruct_cfg.chunk.min(dims.max(glam::UVec3::splat(1))),
            material: self.destruct_cfg.material,
        };
        // Mark surface hits by tri-box SAT in voxel space
        let mut surf = vec![0u8; (dims.x * dims.y * dims.z) as usize];
        let idx =
            |x: u32, y: u32, z: u32| -> usize { (x + y * dims.x + z * dims.x * dims.y) as usize };
        #[inline]
        fn tri_intersects_box(
            a: glam::Vec3,
            b: glam::Vec3,
            c: glam::Vec3,
            center: glam::Vec3,
            half: f32,
        ) -> bool {
            let v0 = a - center;
            let v1 = b - center;
            let v2 = c - center;
            let e0 = v1 - v0;
            let e1 = v2 - v1;
            let e2 = v0 - v2;
            let h = glam::Vec3::splat(half);
            let axes = [
                glam::Vec3::new(0.0, -e0.z, e0.y),
                glam::Vec3::new(0.0, -e1.z, e1.y),
                glam::Vec3::new(0.0, -e2.z, e2.y),
                glam::Vec3::new(e0.z, 0.0, -e0.x),
                glam::Vec3::new(e1.z, 0.0, -e1.x),
                glam::Vec3::new(e2.z, 0.0, -e2.x),
                glam::Vec3::new(-e0.y, e0.x, 0.0),
                glam::Vec3::new(-e1.y, e1.x, 0.0),
                glam::Vec3::new(-e2.y, e2.x, 0.0),
            ];
            for ax in axes.iter() {
                if ax.length_squared() > 1e-12 {
                    let p0 = v0.dot(*ax);
                    let p1 = v1.dot(*ax);
                    let p2 = v2.dot(*ax);
                    let r = h.x * ax.x.abs() + h.y * ax.y.abs() + h.z * ax.z.abs();
                    let minp = p0.min(p1.min(p2));
                    let maxp = p0.max(p1.max(p2));
                    if minp > r || maxp < -r {
                        return false;
                    }
                }
            }
            let minv = glam::Vec3::new(
                v0.x.min(v1.x.min(v2.x)),
                v0.y.min(v1.y.min(v2.y)),
                v0.z.min(v1.z.min(v2.z)),
            );
            let maxv = glam::Vec3::new(
                v0.x.max(v1.x.max(v2.x)),
                v0.y.max(v1.y.max(v2.y)),
                v0.z.max(v1.z.max(v2.z)),
            );
            if minv.x > h.x || maxv.x < -h.x {
                return false;
            }
            if minv.y > h.y || maxv.y < -h.y {
                return false;
            }
            if minv.z > h.z || maxv.z < -h.z {
                return false;
            }
            let n = e0.cross(e1);
            let d = -n.dot(v0);
            let rb = h.x * n.x.abs() + h.y * n.y.abs() + h.z * n.z.abs();
            let s = n.dot(glam::Vec3::ZERO) + d; // plane at origin
            s.abs() <= rb
        }
        // Transform triangles to voxel space
        let to_vox = |p: glam::Vec3| {
            glam::vec3(
                (p.x - bmin.x) / vm,
                (p.y - bmin.y) / vm,
                (p.z - bmin.z) / vm,
            )
        };
        for [a_w, b_w, c_w] in tris.into_iter() {
            let a = to_vox(a_w);
            let b = to_vox(b_w);
            let c = to_vox(c_w);
            let minv = glam::vec3(
                a.x.min(b.x.min(c.x)),
                a.y.min(b.y.min(c.y)),
                a.z.min(b.z.min(c.z)),
            );
            let maxv = glam::vec3(
                a.x.max(b.x.max(c.x)),
                a.y.max(b.y.max(c.y)),
                a.z.max(b.z.max(c.z)),
            );
            let xi0 = minv.x.floor().max(0.0) as u32;
            let yi0 = minv.y.floor().max(0.0) as u32;
            let zi0 = minv.z.floor().max(0.0) as u32;
            let xi1 = maxv.x.ceil().min((dims.x - 1) as f32) as u32;
            let yi1 = maxv.y.ceil().min((dims.y - 1) as f32) as u32;
            let zi1 = maxv.z.ceil().min((dims.z - 1) as f32) as u32;
            for z in zi0..=zi1 {
                for y in yi0..=yi1 {
                    for x in xi0..=xi1 {
                        let center = glam::vec3(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
                        if tri_intersects_box(a, b, c, center, 0.5) {
                            surf[idx(x, y, z)] = 1;
                        }
                    }
                }
            }
        }
        // Fill interior
        // Count surf hits for debug
        let surf_hits = surf.iter().filter(|b| **b != 0).count();
        let grid = voxelize_surface_fill(meta, &surf, self.destruct_cfg.close_surfaces);
        let solid = grid.solid_count();
        log::info!(
            "[destruct] voxelize: ruin={} surf_hits={} solid={} dims={}x{}x{}",
            ruin_idx,
            surf_hits,
            solid,
            grid.dims().x,
            grid.dims().y,
            grid.dims().z
        );
        // Seed colliders
        let dims = grid.dims();
        let csz = grid.meta().chunk;
        let nx = dims.x.div_ceil(csz.x);
        let ny = dims.y.div_ceil(csz.y);
        let nz = dims.z.div_ceil(csz.z);
        let mut colliders = Vec::new();
        for cz in 0..nz {
            for cy in 0..ny {
                for cx in 0..nx {
                    if let Some(sc) =
                        chunkcol::build_chunk_collider(&grid, glam::UVec3::new(cx, cy, cz))
                    {
                        colliders.push(sc);
                    }
                }
            }
        }
        let static_index = Some(chunkcol::rebuild_static_index(&colliders));
        crate::gfx::RuinVox {
            grid,
            chunk_queue: server_core::destructible::queue::ChunkQueue::new(),
            queue_len: 0,
            colliders,
            static_index,
        }
    }

    #[cfg(feature = "legacy_client_carve")]
    #[cfg(any(feature = "legacy_client_carve", feature = "vox_onepath_demo"))]
    fn get_or_spawn_ruin_proxy(&mut self, ruin_idx: usize) -> &mut crate::gfx::RuinVox {
        if !self
            .destr_voxels
            .contains_key(&crate::gfx::DestructibleId(ruin_idx))
        {
            destruct_log!("[destruct] spawn proxy for ruin {}", ruin_idx);
            // IMPORTANT: build the proxy BEFORE hiding the instance, otherwise
            // the zero-scale model matrix would collapse triangles.
            // Prefer real-mesh voxelization when available
            let mut rv = if self.destruct_meshes_cpu.is_empty() {
                let (bmin, bmax) = self.ruin_world_aabb(ruin_idx);
                self.build_ruin_proxy_from_aabb(ruin_idx, bmin, bmax)
            } else {
                self.build_ruin_proxy_from_mesh(ruin_idx)
            };
            // Now hide the original instance visuals
            self.hide_ruins_instance(ruin_idx);
            // Enqueue all chunks and mesh once for immediate appearance
            let dims = rv.grid.dims();
            let csz = rv.grid.meta().chunk;
            let nx = dims.x.div_ceil(csz.x);
            let ny = dims.y.div_ceil(csz.y);
            let nz = dims.z.div_ceil(csz.z);
            let mut enq = 0usize;
            for cz in 0..nz {
                for cy in 0..ny {
                    for cx in 0..nx {
                        rv.chunk_queue.enqueue_many([glam::UVec3::new(cx, cy, cz)]);
                        enq += 1;
                    }
                }
            }
            rv.queue_len = rv.chunk_queue.len();
            destruct_log!("[destruct] queued {} chunks for ruin {}", enq, ruin_idx);
            // Insert proxy before bursting so meshing can find it in the map
            self.destr_voxels
                .insert(crate::gfx::DestructibleId(ruin_idx), rv);
            // Burst a few batches to ensure visibility
            for _ in 0..64 {
                if self
                    .destr_voxels
                    .get(&crate::gfx::DestructibleId(ruin_idx))
                    .is_some_and(|vp| vp.queue_len == 0)
                {
                    break;
                }
                self.process_one_ruin_vox(ruin_idx, 64);
            }
            let total = self
                .voxel_meshes
                .keys()
                .filter(|(id, _, _, _)| id.0 == ruin_idx)
                .count();
            destruct_log!(
                "[destruct] uploaded {} chunk meshes for ruin {} (initial)",
                total,
                ruin_idx
            );
        }
        self.destr_voxels
            .get_mut(&crate::gfx::DestructibleId(ruin_idx))
            .unwrap()
    }

    #[cfg(feature = "legacy_client_carve")]
    #[cfg(any(feature = "legacy_client_carve", feature = "vox_onepath_demo"))]
    fn process_one_ruin_vox(&mut self, ruin_idx: usize, budget: usize) {
        if budget == 0 {
            return;
        }
        let Some(rv) = self
            .destr_voxels
            .get_mut(&crate::gfx::DestructibleId(ruin_idx))
        else {
            return;
        };
        let chunks = rv.chunk_queue.pop_budget(budget);
        if chunks.is_empty() {
            rv.queue_len = rv.chunk_queue.len();
            return;
        }
        let grid = &rv.grid;
        let mut inserted = 0usize;
        let mut removed = 0usize;
        for c in &chunks {
            let key = (crate::gfx::DestructibleId(ruin_idx), c.x, c.y, c.z);
            let h = grid.chunk_occ_hash(*c);
            if self.voxel_hashes.get(&key).copied() == Some(h) {
                continue;
            }
            let mb = voxel_mesh::greedy_mesh_chunk(grid, *c);
            if mb.indices.is_empty() {
                self.voxel_meshes.remove(&key);
                self.voxel_hashes.remove(&key);
                rv.colliders.retain(|sc| sc.coord != *c);
                removed += 1;
            } else {
                let mesh_cpu = ecs_core::components::MeshCpu {
                    positions: mb.positions.clone(),
                    normals: mb.normals.clone(),
                    indices: mb.indices.clone(),
                };
                let _ = crate::gfx::renderer::voxel_upload::upload_chunk_mesh(
                    &self.device,
                    crate::gfx::DestructibleId(ruin_idx),
                    (c.x, c.y, c.z),
                    &mesh_cpu,
                    &mut self.voxel_meshes,
                    &mut self.voxel_hashes,
                );
                // Preserve occupancy-hash skip optimization
                self.voxel_hashes.insert(key, h);
                if let Some(sc) = chunkcol::build_chunk_collider(grid, *c) {
                    chunkcol::swap_in_updates(&mut rv.colliders, vec![sc]);
                    rv.static_index = Some(chunkcol::rebuild_static_index(&rv.colliders));
                }
                inserted += 1;
            }
        }
        rv.queue_len = rv.chunk_queue.len();
        destruct_log!(
            "[destruct] meshed ruin {}: +{} / -{} (queue left={})",
            ruin_idx,
            inserted,
            removed,
            rv.queue_len
        );
    }

    #[cfg(feature = "legacy_client_carve")]
    #[cfg(any(feature = "legacy_client_carve", feature = "vox_onepath_demo"))]
    fn process_all_ruin_queues(&mut self) {
        if self.destr_voxels.is_empty() {
            return;
        }
        let mut remaining = self.destruct_cfg.max_chunk_remesh.max(1);
        if remaining == 0 {
            return;
        }
        let keys: Vec<crate::gfx::DestructibleId> = self.destr_voxels.keys().copied().collect();
        let n = keys.len();
        if n == 0 {
            return;
        }
        let mut idx = 0usize;
        while remaining > 0 {
            let ri = keys[idx % n];
            let before = self
                .destr_voxels
                .get(&ri)
                .map(|rv| rv.queue_len)
                .unwrap_or(0);
            self.process_one_ruin_vox(ri.0, 1);
            let after = self
                .destr_voxels
                .get(&ri)
                .map(|rv| rv.queue_len)
                .unwrap_or(0);
            if after < before {
                remaining = remaining.saturating_sub(1);
            }
            idx += 1;
            if self.destr_voxels.values().all(|rv| rv.queue_len == 0) {
                break;
            }
        }
    }

    #[cfg(feature = "vox_onepath_demo")]
    fn seed_voxel_chunk_colliders(&mut self, grid: &voxel_proxy::VoxelGrid) {
        self.chunk_colliders.clear();
        let d = grid.meta().dims;
        let c = grid.meta().chunk;
        let nx = d.x.div_ceil(c.x);
        let ny = d.y.div_ceil(c.y);
        let nz = d.z.div_ceil(c.z);
        for cz in 0..nz {
            for cy in 0..ny {
                for cx in 0..nx {
                    if let Some(sc) =
                        chunkcol::build_chunk_collider(grid, glam::UVec3::new(cx, cy, cz))
                    {
                        self.chunk_colliders.push(sc);
                    }
                }
            }
        }
        self.static_index = Some(chunkcol::rebuild_static_index(&self.chunk_colliders));
    }

    // (collider refresh done inline at carve sites)

    #[allow(unused_variables)]
    fn explode_fireball_on_segment(
        &mut self,
        owner: Option<usize>,
        p0: glam::Vec3,
        p1: glam::Vec3,
        radius: f32,
        damage: i32,
    ) {
        // visuals + damage
        self.explode_fireball_at(owner, p1, radius, damage);
        #[cfg(feature = "legacy_client_carve")]
        {
            // Voxel impact along the shot segment
            let blast_r = radius * 0.25;
            let mut _handled = false;
            if self.voxel_grid.is_some() {
                let dir = (p1 - p0).normalize_or_zero();
                if dir.length_squared() > 1e-6 {
                    // Gather grid bounds first
                    let (o, vm, d) = {
                        let g = self.voxel_grid.as_ref().unwrap();
                        (g.origin_m(), g.voxel_m().0 as f32, g.dims())
                    };
                    let gmin = glam::vec3(o.x as f32, o.y as f32, o.z as f32);
                    let gmax = gmin + glam::vec3(d.x as f32 * vm, d.y as f32 * vm, d.z as f32 * vm);
                    if let Some((t_enter, _)) = Renderer::ray_box_intersect(p0, dir, gmin, gmax) {
                        let seg_len = (p1 - p0).length();
                        if t_enter <= seg_len {
                            let p_entry = p0 + dir * (t_enter + vm * 1e-3);
                            let max_len_m =
                                core_units::Length::meters(seg_len as f64 + (vm as f64) * 4.0);
                            // Perform DDA and carve while holding the mutable borrow
                            if let Some(hit) = {
                                let g = self.voxel_grid.as_ref().unwrap();
                                raycast_voxels(
                                    g,
                                    glam::DVec3::new(
                                        p_entry.x as f64,
                                        p_entry.y as f64,
                                        p_entry.z as f64,
                                    ),
                                    glam::DVec3::new(dir.x as f64, dir.y as f64, dir.z as f64),
                                    max_len_m,
                                )
                            } {
                                let vc = glam::DVec3::new(
                                    hit.voxel.x as f64 + 0.5,
                                    hit.voxel.y as f64 + 0.5,
                                    hit.voxel.z as f64 + 0.5,
                                );
                                let impact = o + vc * (vm as f64);
                                let out = {
                                    let g = self.voxel_grid.as_mut().unwrap();
                                    carve_and_spawn_debris(
                                        g,
                                        impact,
                                        core_units::Length::meters(blast_r as f64),
                                        self.destruct_cfg.seed ^ self.impact_id,
                                        self.impact_id,
                                        self.destruct_cfg.max_debris,
                                    )
                                };
                                self.impact_id = self.impact_id.wrapping_add(1);
                                let dirty = {
                                    let g = self.voxel_grid.as_mut().unwrap();
                                    g.pop_dirty_chunks(usize::MAX)
                                };
                                // Now refresh colliders and mesh (drop grid borrow before calling self.*)
                                let updates = {
                                    let g = self.voxel_grid.as_ref().unwrap();
                                    dirty
                                        .clone()
                                        .into_iter()
                                        .filter_map(|c| chunkcol::build_chunk_collider(g, c))
                                        .collect::<Vec<_>>()
                                };
                                if !updates.is_empty() {
                                    chunkcol::swap_in_updates(&mut self.chunk_colliders, updates);
                                    self.static_index =
                                        Some(chunkcol::rebuild_static_index(&self.chunk_colliders));
                                }
                                self.chunk_queue.enqueue_many(dirty);
                                self.vox_queue_len = self.chunk_queue.len();
                                let saved = self.destruct_cfg.max_chunk_remesh;
                                self.destruct_cfg.max_chunk_remesh = 32;
                                while self.vox_queue_len > 0 {
                                    self.process_voxel_queues();
                                }
                                self.destruct_cfg.max_chunk_remesh = saved;
                                for (i, p) in out.positions_m.iter().enumerate() {
                                    if (self.debris.len() as u32) >= self.debris_capacity {
                                        break;
                                    }
                                    let pos = glam::vec3(p.x as f32, p.y as f32, p.z as f32);
                                    let vel = out
                                        .velocities_mps
                                        .get(i)
                                        .map(|v| glam::vec3(v.x as f32, v.y as f32, v.z as f32))
                                        .unwrap_or(glam::Vec3::Y * 2.5);
                                    self.debris.push(crate::gfx::Debris {
                                        pos,
                                        vel,
                                        age: 0.0,
                                        life: 2.5,
                                    });
                                }
                                _handled = true;
                            }
                        }
                    }
                }
            }

            // If not handled by an existing grid, do nothing here; we only voxelize on explicit ruin hit.
        }
    }

    #[cfg(feature = "legacy_client_carve")]
    #[allow(dead_code)]
    fn explode_fireball_against_ruin(
        &mut self,
        owner: Option<usize>,
        p0: glam::Vec3,
        p1: glam::Vec3,
        ruin_idx: usize,
        radius: f32,
        damage: i32,
    ) {
        // Visuals + AoE damage centered on p1
        self.explode_fireball_at(owner, p1, radius, damage);
        // Snapshot values to avoid borrowing self during carve
        let seed0 = self.destruct_cfg.seed;
        let impact0 = self.impact_id;
        let max_debris0 = self.destruct_cfg.max_debris;
        // Ensure per‑ruin proxy exists and is meshed (do not hold the &mut)
        let _ = self.get_or_spawn_ruin_proxy(ruin_idx);
        // Carve at current surface via DDA along segment
        let (out_opt, dirty_opt) = {
            if let Some(rv) = self
                .destr_voxels
                .get_mut(&crate::gfx::DestructibleId(ruin_idx))
            {
                let grid = &mut rv.grid;
                let seg = p1 - p0;
                let len = seg.length();
                if len > 1e-6 {
                    let dir = seg / len;
                    let vm = grid.voxel_m().0 as f32;
                    let o = grid.origin_m();
                    let gmin = glam::vec3(o.x as f32, o.y as f32, o.z as f32);
                    let d = grid.dims();
                    let gmax = gmin + glam::vec3(d.x as f32 * vm, d.y as f32 * vm, d.z as f32 * vm);
                    if let Some((t_enter, _)) = Self::ray_box_intersect(p0, dir, gmin, gmax)
                        && t_enter <= len
                    {
                        let p_entry = p0 + dir * (t_enter + vm * 1e-3);
                        if let Some(hit) = server_core::destructible::raycast_voxels(
                            grid,
                            glam::DVec3::new(p_entry.x as f64, p_entry.y as f64, p_entry.z as f64),
                            glam::DVec3::new(dir.x as f64, dir.y as f64, dir.z as f64),
                            core_units::Length::meters((len + vm * 4.0) as f64),
                        ) {
                            let vc = glam::DVec3::new(
                                hit.voxel.x as f64 + 0.5,
                                hit.voxel.y as f64 + 0.5,
                                hit.voxel.z as f64 + 0.5,
                            );
                            let impact = grid.origin_m() + vc * grid.voxel_m().0;
                            let out = server_core::destructible::carve_and_spawn_debris(
                                grid,
                                impact,
                                core_units::Length::meters((radius * 0.25) as f64),
                                seed0 ^ impact0,
                                impact0,
                                max_debris0,
                            );
                            let dirty = grid.pop_dirty_chunks(usize::MAX);
                            (Some(out), Some(dirty))
                        } else {
                            (None, None)
                        }
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            }
        };
        if let Some(out) = out_opt {
            self.impact_id = self.impact_id.wrapping_add(1);
            if let Some(dirty) = dirty_opt
                && !dirty.is_empty()
            {
                if let Some(rv2) = self
                    .destr_voxels
                    .get_mut(&crate::gfx::DestructibleId(ruin_idx))
                {
                    rv2.chunk_queue.enqueue_many(dirty);
                    rv2.queue_len = rv2.chunk_queue.len();
                }
                self.process_one_ruin_vox(ruin_idx, 32);
            }
            for (i, p) in out.positions_m.iter().enumerate() {
                if (self.debris.len() as u32) >= self.debris_capacity {
                    break;
                }
                let pos = glam::vec3(p.x as f32, p.y as f32, p.z as f32);
                let vel = out
                    .velocities_mps
                    .get(i)
                    .map(|v| glam::vec3(v.x as f32, v.y as f32, v.z as f32))
                    .unwrap_or(glam::Vec3::Y * 2.5);
                self.debris.push(crate::gfx::Debris {
                    pos,
                    vel,
                    age: 0.0,
                    life: 2.5,
                });
            }
        }
    }

    #[cfg(feature = "legacy_client_carve")]
    #[cfg(any(feature = "legacy_client_carve", feature = "vox_onepath_demo"))]
    fn hide_ruins_instance(&mut self, index: usize) {
        if index >= self.ruins_instances_cpu.len() {
            return;
        }
        log::info!("[destruct] hide ruins instance {}", index);
        // Zero-scale the instance model matrix to hide it
        let mut inst = self.ruins_instances_cpu[index];
        let m = glam::Mat4::from_scale_rotation_translation(
            glam::Vec3::splat(0.0),
            glam::Quat::IDENTITY,
            glam::Vec3::ZERO,
        );
        inst.model = m.to_cols_array_2d();
        self.ruins_instances_cpu[index] = inst;
        let stride = std::mem::size_of::<crate::gfx::types::Instance>() as u64;
        let offset = (index as u64) * stride;
        let bytes = bytemuck::bytes_of(&inst);
        self.queue
            .write_buffer(&self.ruins_instances, offset, bytes);
    }

    #[cfg(feature = "vox_onepath_demo")]
    fn build_voxel_grid_for_ruins(&mut self, center: glam::Vec3, half_extent: glam::Vec3) {
        // Create a solid box proxy around the ruins instance with clamped size/density
        // 1) Clamp half extents to a sane maximum to avoid huge grids
        let half = glam::vec3(6.0, 5.0, 6.0).min(half_extent.max(glam::Vec3::splat(0.5)));
        // 2) Choose voxel size such that total cells <= ~4M
        let mut vm = self.destruct_cfg.voxel_size_m.0 as f32;
        let size = half * 2.0;
        const MAX_CELLS: u64 = 4_000_000;
        let dims = loop {
            let dx = ((size.x / vm).ceil().max(1.0) as u32).max(8);
            let dy = ((size.y / vm).ceil().max(1.0) as u32).max(8);
            let dz = ((size.z / vm).ceil().max(1.0) as u32).max(8);
            let total = dx as u64 * dy as u64 * dz as u64;
            if total <= MAX_CELLS {
                break glam::UVec3::new(dx, dy, dz);
            }
            vm *= 1.25; // coarsen until under budget
        };
        let origin = glam::DVec3::new(
            (center.x - half.x) as f64,
            (center.y - half.y) as f64,
            (center.z - half.z) as f64,
        );
        let meta = voxel_proxy::VoxelProxyMeta {
            object_id: voxel_proxy::GlobalId(2),
            origin_m: origin,
            voxel_m: core_units::Length::meters(vm as f64),
            dims,
            chunk: self.destruct_cfg.chunk.min(dims.max(glam::UVec3::splat(1))),
            material: self.destruct_cfg.material,
        };
        let mut grid = voxel_proxy::VoxelGrid::new(meta);
        // Fill the entire box solid
        for z in 0..dims.z {
            for y in 0..dims.y {
                for x in 0..dims.x {
                    grid.set(x, y, z, true);
                }
            }
        }
        // Seed colliders before moving grid into self to avoid borrow conflicts
        self.seed_voxel_chunk_colliders(&grid);
        self.voxel_grid = Some(grid);
        // enqueue all chunks
        let d = dims;
        let c = self.destruct_cfg.chunk;
        let nx = d.x.div_ceil(c.x);
        let ny = d.y.div_ceil(c.y);
        let nz = d.z.div_ceil(c.z);
        self.chunk_queue = server_core::destructible::queue::ChunkQueue::new();
        for cz in 0..nz {
            for cy in 0..ny {
                for cx in 0..nx {
                    self.chunk_queue
                        .enqueue_many([glam::UVec3::new(cx, cy, cz)]);
                }
            }
        }
        self.vox_queue_len = self.chunk_queue.len();
        // Process aggressively so geometry shows immediately
        let saved = self.destruct_cfg.max_chunk_remesh;
        self.destruct_cfg.max_chunk_remesh = 64;
        while self.vox_queue_len > 0 {
            self.process_voxel_queues();
        }
        self.destruct_cfg.max_chunk_remesh = saved;
    }
    #[inline]
    pub(crate) fn wrap_angle(a: f32) -> f32 {
        let mut x = a;
        while x > std::f32::consts::PI {
            x -= 2.0 * std::f32::consts::PI;
        }
        while x < -std::f32::consts::PI {
            x += 2.0 * std::f32::consts::PI;
        }
        x
    }
    // update_player_and_camera removed: moved to client_runtime::SceneInputs

    pub(crate) fn apply_pc_transform(&mut self) {
        if !self.pc_alive || self.pc_index >= self.wizard_count as usize {
            return;
        }
        // Update CPU model matrix and upload only the PC instance
        let rot = glam::Quat::from_rotation_y(self.player.yaw);
        // Project player onto terrain height
        let (h, _n) = terrain::height_at(&self.terrain_cpu, self.player.pos.x, self.player.pos.z);
        let pos = glam::vec3(self.player.pos.x, h, self.player.pos.z);
        let m = glam::Mat4::from_scale_rotation_translation(glam::Vec3::splat(1.0), rot, pos);
        self.wizard_models[self.pc_index] = m;
        let mut inst = self.wizard_instances_cpu[self.pc_index];
        inst.model = m.to_cols_array_2d();
        self.wizard_instances_cpu[self.pc_index] = inst;
        let offset = (self.pc_index * std::mem::size_of::<InstanceSkin>()) as u64;
        self.queue
            .write_buffer(&self.wizard_instances, offset, bytemuck::bytes_of(&inst));
    }

    pub(crate) fn update_wizard_palettes(&mut self, time_global: f32) {
        // Build palettes for each wizard with its animation + offset.
        if self.wizard_count == 0 {
            return;
        }
        let joints = self.joints_per_wizard as usize;
        let mut mats: Vec<glam::Mat4> = Vec::with_capacity(self.wizard_count as usize * joints);
        for i in 0..(self.wizard_count as usize) {
            let clip = self.select_clip(self.wizard_anim_index[i]);
            let palette = if self.pc_alive
                && i == self.pc_index
                && self.pc_index < self.wizard_count as usize
            {
                if let Some(start) = self.pc_anim_start {
                    let lt = (time_global - start).clamp(0.0, clip.duration.max(0.0));
                    anim::sample_palette(&self.skinned_cpu, clip, lt)
                } else {
                    anim::sample_palette(&self.skinned_cpu, clip, time_global)
                }
            } else {
                let t = time_global + self.wizard_time_offset[i];
                anim::sample_palette(&self.skinned_cpu, clip, t)
            };
            mats.extend(palette);
        }
        // Upload as raw f32x16
        let mut raw: Vec<[f32; 16]> = Vec::with_capacity(mats.len());
        for m in mats {
            raw.push(m.to_cols_array());
        }
        self.queue
            .write_buffer(&self.palettes_buf, 0, bytemuck::cast_slice(&raw));
    }

    pub(crate) fn select_clip(&self, idx: usize) -> &AnimClip {
        // Honor the requested clip first; fallback only if missing.
        let requested = match idx {
            0 => "PortalOpen",
            1 => "Still",
            _ => "Waiting",
        };
        if let Some(c) = self.skinned_cpu.animations.get(requested) {
            return c;
        }
        for name in ["Waiting", "Still", "PortalOpen"] {
            if let Some(c) = self.skinned_cpu.animations.get(name) {
                return c;
            }
        }
        self.skinned_cpu
            .animations
            .values()
            .next()
            .expect("at least one animation clip present")
    }

    pub(crate) fn process_pc_cast(&mut self, t: f32) {
        if !self.pc_alive || self.pc_index >= self.wizard_count as usize {
            return;
        }
        if self.pc_cast_queued {
            self.pc_cast_queued = false;
            if self.wizard_anim_index[self.pc_index] != 0 && self.pc_anim_start.is_none() {
                // Start PortalOpen now
                self.wizard_anim_index[self.pc_index] = 0;
                self.wizard_time_offset[self.pc_index] = -t; // phase=0 at start
                self.wizard_last_phase[self.pc_index] = 0.0;
                self.pc_anim_start = Some(t);
                self.pc_cast_fired = false;
            }
        }
        if let Some(start) = self.pc_anim_start {
            if self.wizard_anim_index[self.pc_index] == 0 {
                let clip = self.select_clip(0);
                let elapsed = t - start;
                // Fire exactly at cast end if not yet fired
                if !self.pc_cast_fired && elapsed >= self.pc_cast_time {
                    let phase = self.pc_cast_time;
                    if let Some(origin_local) = self.right_hand_world(clip, phase) {
                        let inst = self
                            .wizard_models
                            .get(self.pc_index)
                            .copied()
                            .unwrap_or(glam::Mat4::IDENTITY);
                        let origin_w = inst
                            * glam::Vec4::new(origin_local.x, origin_local.y, origin_local.z, 1.0);
                        let dir_w = (inst * glam::Vec4::new(0.0, 0.0, 1.0, 0.0))
                            .truncate()
                            .normalize_or_zero();
                        let right_w = (inst * glam::Vec4::new(1.0, 0.0, 0.0, 0.0))
                            .truncate()
                            .normalize_or_zero();
                        let lateral = 0.20;
                        let spawn = origin_w.truncate() + dir_w * 0.3 - right_w * lateral;
                        match self.pc_cast_kind.unwrap_or(super::super::PcCast::FireBolt) {
                            super::super::PcCast::FireBolt => {
                                let fb_col = [2.6, 0.7, 0.18];
                                self.spawn_firebolt(
                                    spawn,
                                    dir_w,
                                    t,
                                    Some(self.pc_index),
                                    false,
                                    fb_col,
                                );
                                // Start cooldown via SceneInputs (single source of truth)
                                let spell_id = "wiz.fire_bolt.srd521";
                                self.scene_inputs.start_cooldown(
                                    spell_id,
                                    self.last_time,
                                    self.firebolt_cd_dur,
                                );
                            }
                            super::super::PcCast::MagicMissile => {
                                self.spawn_magic_missile(spawn, dir_w, t);
                                // Start cooldown via SceneInputs
                                let spell_id = "wiz.magic_missile.srd521";
                                self.scene_inputs.start_cooldown(
                                    spell_id,
                                    self.last_time,
                                    self.magic_missile_cd_dur,
                                );
                            }
                            super::super::PcCast::Fireball => {
                                self.spawn_fireball(spawn, dir_w, t, Some(self.pc_index));
                                let spell_id = "wiz.fireball.srd521";
                                self.scene_inputs.start_cooldown(
                                    spell_id,
                                    self.last_time,
                                    self.fireball_cd_dur,
                                );
                            }
                        }
                        self.pc_cast_fired = true;
                    }
                    // End cast animation and start cooldown window
                    self.wizard_anim_index[self.pc_index] = 1;
                    self.pc_anim_start = None;
                }
            } else {
                self.pc_anim_start = None;
            }
        }
    }

    /// Update and render-side state for projectiles/particles
    pub(crate) fn update_fx(&mut self, t: f32, dt: f32) {
        // 1) Spawn firebolts for PortalOpen phase crossing (NPC wizards only).
        if self.wizard_count > 0 {
            let zombies_alive = self.any_zombies_alive();
            let cycle = 5.0f32; // synthetic cycle period
            let bolt_offset = 1.5f32; // trigger point in the cycle
            for i in 0..(self.wizard_count as usize) {
                if self.wizard_anim_index[i] != 0 {
                    continue;
                }
                let prev = self.wizard_last_phase[i];
                let phase = (t + self.wizard_time_offset[i]) % cycle;
                let crossed = (prev <= bolt_offset && phase >= bolt_offset)
                    || (prev > phase && (prev <= bolt_offset || phase >= bolt_offset));
                // If wizards have aggroed on the player, they may fire even without zombies present
                let allowed = i == self.pc_index || zombies_alive || self.wizards_hostile_to_pc;
                if allowed && crossed && i != self.pc_index {
                    let clip = self.select_clip(self.wizard_anim_index[i]);
                    let clip_time = if clip.duration > 0.0 {
                        phase.min(clip.duration)
                    } else {
                        0.0
                    };
                    if let Some(origin_local) = self.right_hand_world(clip, clip_time) {
                        let inst = self
                            .wizard_models
                            .get(i)
                            .copied()
                            .unwrap_or(glam::Mat4::IDENTITY);
                        let origin_w = inst
                            * glam::Vec4::new(origin_local.x, origin_local.y, origin_local.z, 1.0);
                        let dir_w = (inst * glam::Vec4::new(0.0, 0.0, 1.0, 0.0))
                            .truncate()
                            .normalize_or_zero();
                        let right_w = (inst * glam::Vec4::new(1.0, 0.0, 0.0, 0.0))
                            .truncate()
                            .normalize_or_zero();
                        let lateral = 0.20;
                        let spawn = origin_w.truncate() + dir_w * 0.3 - right_w * lateral;
                        // Decide between Fire Bolt (default) and Fireball (occasional, far targets only)
                        let min_fireball_dist = 10.0f32; // meters
                        let mut target_dist = f32::INFINITY;
                        if self.wizards_hostile_to_pc && self.pc_alive {
                            if let Some(pm) = self.wizard_models.get(self.pc_index) {
                                let c = pm.to_cols_array();
                                let pc = glam::vec3(c[12], c[13], c[14]);
                                let wpos = (inst * glam::Vec4::new(0.0, 0.0, 0.0, 1.0)).truncate();
                                target_dist = (pc - wpos).length();
                            }
                        } else {
                            let wpos = (inst * glam::Vec4::new(0.0, 0.0, 0.0, 1.0)).truncate();
                            for n in &self.server.npcs {
                                if !n.alive {
                                    continue;
                                }
                                let d = glam::vec2(n.pos.x - wpos.x, n.pos.z - wpos.z).length();
                                if d < target_dist {
                                    target_dist = d;
                                }
                            }
                        }
                        let mut use_fireball = false;
                        if target_dist.is_finite()
                            && target_dist >= min_fireball_dist
                            && let Some(cnt) = self.wizard_fire_cycle_count.get_mut(i)
                        {
                            *cnt += 1;
                            let next_at = self.wizard_fireball_next_at.get(i).copied().unwrap_or(4);
                            if *cnt >= next_at {
                                use_fireball = true;
                                *cnt = 0;
                                // roll next threshold 3..=5
                                let mut r = rand::rng();
                                let tnext: u32 = r.random_range(3..=5);
                                if let Some(slot) = self.wizard_fireball_next_at.get_mut(i) {
                                    *slot = tnext;
                                }
                            }
                        }
                        if use_fireball {
                            self.spawn_fireball(spawn, dir_w, t, Some(i));
                        } else {
                            let fb_col = [2.6, 0.7, 0.18];
                            self.spawn_firebolt(spawn, dir_w, t, Some(i), true, fb_col);
                        }
                    }
                }
                self.wizard_last_phase[i] = phase;
            }
        }

        // 2) Integrate projectiles and keep them slightly above ground
        let ground_clearance = 0.15f32; // meters above terrain
        for p in &mut self.projectiles {
            p.pos += p.vel * dt;
            p.pos = gfx::util::clamp_above_terrain(&self.terrain_cpu, p.pos, ground_clearance);
        }
        // 2.5) Fireball collisions (custom AoE explode on hit)
        if !self.projectiles.is_empty() {
            let mut i = 0usize;
            while i < self.projectiles.len() {
                let pr = self.projectiles[i];
                if let crate::gfx::fx::ProjectileKind::Fireball { radius, damage } = pr.kind {
                    let p0 = pr.pos - pr.vel * dt;
                    let p1 = pr.pos;
                    if let Some((_did, _t)) = self.find_destructible_hit(p0, p1) {
                        destruct_log!("[destruct] hit did={:?}", _did);
                        #[cfg(feature = "legacy_client_carve")]
                        {
                            self.explode_fireball_against_destructible(
                                pr.owner_wizard,
                                p0,
                                p1,
                                _did,
                                _t,
                                radius,
                                damage,
                            );
                            self.projectiles.swap_remove(i);
                            continue;
                        }
                        #[cfg(not(feature = "legacy_client_carve"))]
                        {
                            // Default: still show explosion visuals on destructible hit
                            self.explode_fireball_at(pr.owner_wizard, p1, radius, damage);
                            self.projectiles.swap_remove(i);
                            continue;
                        }
                    }
                    let mut exploded = false;
                    // collide against any alive NPC cylinder in XZ
                    if !self.server.npcs.is_empty() {
                        for n in &self.server.npcs {
                            if !n.alive {
                                continue;
                            }
                            if segment_hits_circle_xz(p0, p1, n.pos, n.radius) {
                                exploded = true;
                                break;
                            }
                        }
                    }
                    // voxel world collision vs chunk colliders (AABB test)
                    if !exploded && !self.chunk_colliders.is_empty() {
                        let d = p1 - p0;
                        let seg_len = d.length();
                        if seg_len > 1e-6 {
                            let mut hit_any = false;
                            for ch in &self.chunk_colliders {
                                let aabb = ch.collider.aabb;
                                if let Some((t_enter, _t_exit)) =
                                    Self::ray_box_intersect(p0, d, aabb.min, aabb.max)
                                    && t_enter >= 0.0
                                    && t_enter <= seg_len
                                {
                                    hit_any = true;
                                    break;
                                }
                            }
                            if hit_any {
                                self.explode_fireball_on_segment(
                                    pr.owner_wizard,
                                    p0,
                                    p1,
                                    radius,
                                    damage,
                                );
                                self.projectiles.swap_remove(i);
                                continue;
                            }
                        }
                    }
                    if exploded {
                        self.explode_fireball_on_segment(pr.owner_wizard, p0, p1, radius, damage);
                        self.projectiles.swap_remove(i);
                        continue;
                    }
                }
                i += 1;
            }
        }
        // 2.55) Server-side collision vs NPCs (normal single-hit projectiles)
        if !self.projectiles.is_empty() && !self.server.npcs.is_empty() {
            let damage = 10; // TODO: integrate with spell spec dice
            let hits = self
                .server
                .collide_and_damage(&mut self.projectiles, dt, damage);
            for h in &hits {
                // Impact burst at hit position
                for _ in 0..16 {
                    let a = rand_unit() * std::f32::consts::TAU;
                    let r = 4.0 + rand_unit() * 1.2;
                    self.particles.push(Particle {
                        pos: h.pos,
                        vel: glam::vec3(a.cos() * r, 2.0 + rand_unit() * 1.2, a.sin() * r),
                        age: 0.0,
                        life: 0.18,
                        size: 0.02,
                        color: [1.7, 0.85, 0.35],
                    });
                }
                // Damage floater above NPC head (terrain/instance-aware)
                // 1) Death Knight (handle first so we can despawn on fatal)
                if self.dk_id.is_some() && self.dk_id.unwrap() == h.npc {
                    // Spawn damage near DK head using its model matrix if present
                    if let Some(m) = self.dk_models.first().copied() {
                        let head = m * glam::Vec4::new(0.0, 1.6, 0.0, 1.0);
                        self.damage.spawn(head.truncate(), h.damage);
                    } else {
                        self.damage
                            .spawn(h.pos + glam::vec3(0.0, 1.2, 0.0), h.damage);
                    }
                    // If fatal, hide the DK instance and clear id
                    if h.fatal {
                        self.dk_count = 0;
                        self.dk_id = None;
                    }
                } else if let Some(idx) = self.zombie_ids.iter().position(|id| *id == h.npc) {
                    let m = self
                        .zombie_models
                        .get(idx)
                        .copied()
                        .unwrap_or(glam::Mat4::IDENTITY);
                    let head = m * glam::Vec4::new(0.0, 1.6, 0.0, 1.0);
                    self.damage.spawn(head.truncate(), h.damage);
                    // Remove zombie visuals if fatal
                    if h.fatal {
                        self.zombie_ids.swap_remove(idx);
                        self.zombie_models.swap_remove(idx);
                        if (idx as u32) < self.zombie_count {
                            self.zombie_instances_cpu.swap_remove(idx);
                            self.zombie_count -= 1;
                            // Recompute palette_base for contiguity
                            for (i, inst) in self.zombie_instances_cpu.iter_mut().enumerate() {
                                inst.palette_base = (i as u32) * self.zombie_joints;
                            }
                            let bytes: &[u8] = bytemuck::cast_slice(&self.zombie_instances_cpu);
                            self.queue.write_buffer(&self.zombie_instances, 0, bytes);
                        }
                    }
                } else if let Some(n) = self.server.npcs.iter().find(|n| n.id == h.npc) {
                    let (hgt, _n) = terrain::height_at(&self.terrain_cpu, n.pos.x, n.pos.z);
                    let pos = glam::vec3(n.pos.x, hgt + n.radius + 0.9, n.pos.z);
                    self.damage.spawn(pos, h.damage);
                } else {
                    self.damage
                        .spawn(h.pos + glam::vec3(0.0, 0.9, 0.0), h.damage);
                    let (hgt, _n) = terrain::height_at(&self.terrain_cpu, h.pos.x, h.pos.z);
                    self.damage
                        .spawn(glam::vec3(h.pos.x, hgt + 0.9, h.pos.z), h.damage);
                }
            }
        }
        // Ground hit or timeout
        let mut burst: Vec<Particle> = Vec::new();
        let mut i = 0;
        while i < self.projectiles.len() {
            let kill = self.last_time >= self.projectiles[i].t_die;
            if kill {
                let hit = self.projectiles[i].pos;
                // If Fireball, explode on timeout at current position
                if let crate::gfx::fx::ProjectileKind::Fireball { radius, damage } =
                    self.projectiles[i].kind
                {
                    let owner = self.projectiles[i].owner_wizard;
                    let p0 = hit - self.projectiles[i].vel * dt;
                    self.explode_fireball_on_segment(owner, p0, hit, radius, damage);
                }
                // small flare + compact burst
                burst.push(Particle {
                    pos: hit,
                    vel: glam::Vec3::ZERO,
                    age: 0.0,
                    life: 0.12,
                    size: 0.06,
                    color: [1.8, 1.2, 0.4],
                });
                for _ in 0..10 {
                    let a = rand_unit() * std::f32::consts::TAU;
                    let r = 3.0 + rand_unit() * 0.8;
                    burst.push(Particle {
                        pos: hit,
                        vel: glam::vec3(a.cos() * r, 1.5 + rand_unit() * 1.0, a.sin() * r),
                        age: 0.0,
                        life: 0.12,
                        size: 0.015,
                        color: [1.6, 0.9, 0.3],
                    });
                }
                self.projectiles.swap_remove(i);
            } else {
                i += 1;
            }
        }
        if !burst.is_empty() {
            self.particles.append(&mut burst);
        }

        // 2.6) Collide with wizards/PC (friendly fire on)
        if !self.projectiles.is_empty() {
            self.collide_with_wizards(dt, 10);
        }

        // 3) Simulate impact particles (age, simple gravity, fade)
        let cam = self.cam_follow.current_pos;
        let max_d2 = 400.0 * 400.0; // cull far particles
        let mut j = 0usize;
        while j < self.particles.len() {
            let p = &mut self.particles[j];
            p.age += dt;
            p.vel.y -= 9.8 * dt * 0.5;
            p.vel *= 0.98f32.powf(dt.max(0.0) * 60.0);
            p.pos += p.vel * dt;
            if p.age >= p.life || (p.pos - cam).length_squared() > max_d2 {
                self.particles.swap_remove(j);
                continue;
            }
            j += 1;
        }

        // 4) Upload FX instances (billboard particles)
        let mut inst: Vec<ParticleInstance> =
            Vec::with_capacity(self.projectiles.len() * 3 + self.particles.len());
        for pr in &self.projectiles {
            // Fade head near lifetime end
            let mut head_fade = 1.0f32;
            let fade_window = 0.15f32;
            if pr.t_die > 0.0 {
                let remain = (pr.t_die - t).max(0.0);
                head_fade = (remain / fade_window).clamp(0.0, 1.0);
            }
            // Make Fireball visuals bigger and brighter
            let (head_size, trail_size, bright_mul) = match pr.kind {
                crate::gfx::fx::ProjectileKind::Fireball { .. } => (0.36, 0.26, 2.0),
                _ => (0.18, 0.13, 1.0),
            };
            // head
            inst.push(ParticleInstance {
                pos: [pr.pos.x, pr.pos.y, pr.pos.z],
                size: head_size,
                color: [
                    pr.color[0] * bright_mul * head_fade,
                    pr.color[1] * bright_mul * head_fade,
                    pr.color[2] * bright_mul * head_fade,
                ],
                _pad: 0.0,
            });
            // short trail segments behind
            let dir = pr.vel.normalize_or_zero();
            for k in 1..=2 {
                let tseg = k as f32 * 0.02;
                let p = pr.pos - dir * (tseg * pr.vel.length());
                let fade = (1.0 - (k as f32) * 0.35) * head_fade;
                inst.push(ParticleInstance {
                    pos: [p.x, p.y, p.z],
                    size: trail_size,
                    color: [
                        pr.color[0] * 0.8 * bright_mul * fade,
                        pr.color[1] * 0.8 * bright_mul * fade,
                        pr.color[2] * 0.8 * bright_mul * fade,
                    ],
                    _pad: 0.0,
                });
            }
        }
        // Impacts
        for p in &self.particles {
            let f = 1.0 - (p.age / p.life).clamp(0.0, 1.0);
            let size = p.size * (1.0 + 0.5 * (1.0 - f));
            inst.push(ParticleInstance {
                pos: [p.pos.x, p.pos.y, p.pos.z],
                size,
                color: [
                    p.color[0] * f * 1.5,
                    p.color[1] * f * 1.5,
                    p.color[2] * f * 1.5,
                ],
                _pad: 0.0,
            });
        }
        if (inst.len() as u32) > self._fx_capacity {
            inst.truncate(self._fx_capacity as usize);
        }
        self.fx_count = inst.len() as u32;
        if self.fx_count > 0 {
            self.queue
                .write_buffer(&self.fx_instances, 0, bytemuck::cast_slice(&inst));
        }

        // 5) If no zombies remain, retire NPC wizards from the casting loop unless hostile to player
        if !self.any_zombies_alive() && !self.wizards_hostile_to_pc {
            for i in 0..(self.wizard_count as usize) {
                if i == self.pc_index {
                    continue;
                }
                if self.wizard_anim_index[i] == 0 {
                    self.wizard_anim_index[i] = 2;
                }
            }
        }
    }

    pub(crate) fn collide_with_wizards(&mut self, dt: f32, damage: i32) {
        let mut i = 0usize;
        while i < self.projectiles.len() {
            let pr = self.projectiles[i];
            let p0 = pr.pos - pr.vel * dt;
            let p1 = pr.pos;
            let mut hit_someone = false;
            for j in 0..(self.wizard_count as usize) {
                if Some(j) == pr.owner_wizard {
                    continue;
                } // do not hit the caster
                let hp = self.wizard_hp.get(j).copied().unwrap_or(self.wizard_hp_max);
                if hp <= 0 {
                    continue;
                }
                let m = self.wizard_models[j].to_cols_array();
                let center = glam::vec3(m[12], m[13], m[14]);
                let r = 0.7f32; // generous cylinder radius
                if segment_hits_circle_xz(p0, p1, center, r) {
                    let before = self.wizard_hp[j];
                    let after = (before - damage).max(0);
                    self.wizard_hp[j] = after;
                    let fatal = after == 0;
                    // Floating damage number
                    let head = center + glam::vec3(0.0, 1.7, 0.0);
                    self.damage.spawn(head, damage);
                    // If the player hit any wizard, all wizards become hostile to the player
                    if pr.owner_wizard == Some(self.pc_index) {
                        self.wizards_hostile_to_pc = true;
                        // Ensure NPC wizards resume casting loop even if all monsters are dead
                        // by switching them back to the PortalOpen loop.
                        for i in 0..(self.wizard_count as usize) {
                            if i == self.pc_index {
                                continue;
                            }
                            if self.wizard_hp.get(i).copied().unwrap_or(0) <= 0 {
                                continue;
                            }
                            if self.wizard_anim_index[i] != 0 {
                                self.wizard_anim_index[i] = 0;
                                // Reset last phase so they can fire promptly
                                self.wizard_last_phase[i] = 0.0;
                            }
                        }
                    }
                    if fatal {
                        if j == self.pc_index {
                            self.kill_pc();
                        } else {
                            self.remove_wizard_at(j);
                        }
                    }
                    // impact burst
                    for _ in 0..14 {
                        let a = rand_unit() * std::f32::consts::TAU;
                        let r2 = 3.5 + rand_unit() * 1.0;
                        self.particles.push(Particle {
                            pos: p1,
                            vel: glam::vec3(a.cos() * r2, 2.0 + rand_unit() * 1.0, a.sin() * r2),
                            age: 0.0,
                            life: 0.16,
                            size: 0.02,
                            color: [1.8, 0.8, 0.3],
                        });
                    }
                    self.projectiles.swap_remove(i);
                    hit_someone = true;
                    break;
                }
            }
            if !hit_someone {
                i += 1;
            }
        }

        // 2.6) Projectiles that died without hitting an NPC: attempt voxel impact (Fireball only)
        let mut i = 0usize;
        while i < self.projectiles.len() {
            let kill = self.last_time >= self.projectiles[i].t_die;
            if kill {
                let p1 = self.projectiles[i].pos;
                let p0 = p1 - self.projectiles[i].vel * dt.max(1e-3);
                if let crate::gfx::fx::ProjectileKind::Fireball { .. } = self.projectiles[i].kind {
                    self.try_voxel_impact(p0, p1);
                }
                self.projectiles.swap_remove(i);
            } else {
                i += 1;
            }
        }

        // 2.7) Process voxel chunk work budget per frame
        // Multi‑proxy: process all ruin queues, then the single‑grid demo if present
        #[cfg(feature = "legacy_client_carve")]
        self.process_all_ruin_queues();
        #[cfg(feature = "vox_onepath_demo")]
        self.process_voxel_queues();
    }

    #[cfg(feature = "legacy_client_carve")]
    pub(crate) fn try_voxel_impact(&mut self, p0: glam::Vec3, p1: glam::Vec3) {
        let is_demo = self.is_vox_onepath();
        let Some(grid) = self.voxel_grid.as_mut() else {
            return;
        };
        // Trace strictly along the projectile segment (plus a small safety margin)
        let seg = p1 - p0;
        if seg.length_squared() < 1e-6 {
            return;
        }
        let dir = seg.normalize_or_zero();
        let origin = DVec3::new(p0.x as f64, p0.y as f64, p0.z as f64);
        let dir_m = DVec3::new(dir.x as f64, dir.y as f64, dir.z as f64);
        // Extend a bit beyond the segment to catch grazing hits
        let max_len_m = core_units::Length::meters((seg.length() * 1.25) as f64);
        // Extra guard: skip raycast entirely if the projectile segment doesn't intersect
        // the grid AABB (in meters). Prevents accidental carves when firing into empty sky.
        {
            let vm_f = grid.voxel_m().0 as f32;
            let dims = grid.dims();
            let gmin = glam::vec3(
                grid.origin_m().x as f32,
                grid.origin_m().y as f32,
                grid.origin_m().z as f32,
            );
            let gmax = gmin
                + glam::vec3(
                    dims.x as f32 * vm_f,
                    dims.y as f32 * vm_f,
                    dims.z as f32 * vm_f,
                );
            let aabb_min = gmin - glam::Vec3::splat(0.25 * vm_f);
            let aabb_max = gmax + glam::Vec3::splat(0.25 * vm_f);
            let mut tmin = 0.0f32;
            let mut tmax = 1.0f32;
            let d = p1 - p0;
            for i in 0..3 {
                let s = p0[i];
                let dir = d[i];
                let minb = aabb_min[i];
                let maxb = aabb_max[i];
                if dir.abs() < 1e-6 {
                    if s < minb || s > maxb {
                        return; // parallel and outside slab
                    }
                } else {
                    let inv = 1.0 / dir;
                    let mut t0 = (minb - s) * inv;
                    let mut t1 = (maxb - s) * inv;
                    if t0 > t1 {
                        core::mem::swap(&mut t0, &mut t1);
                    }
                    tmin = tmin.max(t0);
                    tmax = tmax.min(t1);
                    if tmin > tmax {
                        return; // no intersection
                    }
                }
            }
        }
        if let Some(hit) = raycast_voxels(grid, origin, dir_m, max_len_m) {
            // Carve a small hole at voxel center and schedule chunk updates
            let vm = grid.voxel_m().0;
            let o = grid.origin_m();
            let vc = DVec3::new(
                hit.voxel.x as f64 + 0.5,
                hit.voxel.y as f64 + 0.5,
                hit.voxel.z as f64 + 0.5,
            );
            let impact = o + vc * vm;
            // Demo: jitter radius per impact when in vox_onepath mode; otherwise use default
            let mut radius = if is_demo {
                let mut rng =
                    self.destruct_cfg.seed ^ self.impact_id.wrapping_mul(0x9E37_79B9_7F4A_7C15);
                let r_m = lerp(0.22, 0.45, rand01(&mut rng)) as f64;
                core_units::Length::meters(r_m)
            } else {
                self.destruct_cfg.voxel_size_m * 2.0
            };
            // Guardrail: clamp radius so chunks touched <= max_carve_chunks
            if let Some(maxc) = self.destruct_cfg.max_carve_chunks {
                let mut tries = 0;
                loop {
                    let vm = grid.voxel_m().0;
                    let r = radius.0 as f32;
                    let o = grid.origin_m();
                    let c_v = ((impact - o) / vm).as_vec3();
                    let d = grid.dims();
                    let csz = grid.meta().chunk;
                    // compute chunk bounds of sphere AABB
                    let min_v = (c_v - glam::Vec3::splat(r / vm as f32))
                        .floor()
                        .max(glam::Vec3::ZERO);
                    let max_v = (c_v + glam::Vec3::splat(r / vm as f32)).ceil();
                    let cx0 = (min_v.x as u32 / csz.x).min(d.x.saturating_sub(1) / csz.x);
                    let cy0 = (min_v.y as u32 / csz.y).min(d.y.saturating_sub(1) / csz.y);
                    let cz0 = (min_v.z as u32 / csz.z).min(d.z.saturating_sub(1) / csz.z);
                    let cx1 = (max_v.x.max(0.0) as u32 / csz.x).min(d.x.saturating_sub(1) / csz.x);
                    let cy1 = (max_v.y.max(0.0) as u32 / csz.y).min(d.y.saturating_sub(1) / csz.y);
                    let cz1 = (max_v.z.max(0.0) as u32 / csz.z).min(d.z.saturating_sub(1) / csz.z);
                    let count = (cx1.saturating_sub(cx0) + 1) as u64
                        * (cy1.saturating_sub(cy0) + 1) as u64
                        * (cz1.saturating_sub(cz0) + 1) as u64;
                    if count as u32 <= maxc || tries > 5 {
                        break;
                    }
                    radius *= 0.85; // shrink and retry
                    tries += 1;
                }
            }
            log::info!(
                "[vox] hit @ ({:.2},{:.2},{:.2}) r={:.2}m",
                impact.x,
                impact.y,
                impact.z,
                radius.0
            );
            // Demo: vary seed and per-impact debris cap in vox_onepath mode
            let (seed, max_debris_hit) = if is_demo {
                let mut rng =
                    self.destruct_cfg.seed ^ self.impact_id.wrapping_mul(0xA24B_A1AC_B9F1_3F7B);
                let debris_scale = lerp(0.60, 1.40, rand01(&mut rng));
                let cap = ((self.destruct_cfg.max_debris as f32 * debris_scale).round() as u32)
                    .max(8) as usize;
                let seed = splitmix64(&mut rng);
                (seed, cap)
            } else {
                (self.destruct_cfg.seed, self.destruct_cfg.max_debris)
            };
            let out =
                carve_and_spawn_debris(grid, impact, radius, seed, self.impact_id, max_debris_hit);
            // Optional: append JSONL replay record (native builds only)
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(ref path) = self.destruct_cfg.replay_log {
                let _ = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .and_then(|mut f| {
                        use std::io::Write as _;
                        let line = format!(
                            "{{\"impact_id\":{},\"center\":[{:.6},{:.6},{:.6}],\"radius\":{:.6}}}\n",
                            self.impact_id,
                            impact.x, impact.y, impact.z,
                            (self.destruct_cfg.voxel_size_m * 2.0).0
                        );
                        f.write_all(line.as_bytes())
                    });
            }
            self.impact_id = self.impact_id.wrapping_add(1);
            self.vox_debris_last = out.positions_m.len();
            // Stash recent impacts (for quick replay)
            if !out.positions_m.is_empty() {
                let rec = (impact, radius.0);
                if self.recent_impacts.len() >= 3 {
                    self.recent_impacts.remove(0);
                }
                self.recent_impacts.push(rec);
            }
            // Spawn visible debris instances (cubes)
            let _vsize = grid.voxel_m().0 as f32;
            for (i, p) in out.positions_m.iter().enumerate() {
                let pos = glam::vec3(p.x as f32, p.y as f32, p.z as f32);
                let vel = out
                    .velocities_mps
                    .get(i)
                    .map(|v| glam::vec3(v.x as f32, v.y as f32, v.z as f32))
                    .unwrap_or(glam::Vec3::Y * 2.5);
                if (self.debris.len() as u32) < self.debris_capacity {
                    self.debris.push(crate::gfx::Debris {
                        pos,
                        vel,
                        age: 0.0,
                        life: 2.5,
                    });
                }
            }
            // Enqueue chunks deterministically
            let enq = grid.pop_dirty_chunks(usize::MAX);
            self.chunk_queue.enqueue_many(enq);
            self.vox_queue_len = self.chunk_queue.len();
        } else {
            // No voxel hit along the projectile path — do nothing
        }
    }

    #[cfg(feature = "vox_onepath_demo")]
    pub fn process_voxel_queues(&mut self) {
        // In the one‑path demo, burst‑remesh to make cuts instantly visible
        let budget = if self.vox_onepath_ui.is_some() {
            64
        } else {
            self.destruct_cfg.max_chunk_remesh.max(1)
        };
        let chunks = self.chunk_queue.pop_budget(budget);
        if let Some(grid) = self.voxel_grid.as_ref() {
            let t0 = Instant::now();
            // Mesh changed chunks and upload to GPU; drop entries that became empty
            let mut skipped = 0usize;
            for c in &chunks {
                // Skip meshing if occupancy hash hasn't changed
                let key = (crate::gfx::DestructibleId(0), c.x, c.y, c.z);
                let h = grid.chunk_occ_hash(*c);
                if self.vox_onepath_ui.is_none() && self.voxel_hashes.get(&key).copied() == Some(h)
                {
                    skipped += 1;
                    continue;
                }
                let mb = if self.vox_onepath_ui.is_some() {
                    voxel_mesh::naive_mesh_chunk(grid, *c)
                } else {
                    voxel_mesh::greedy_mesh_chunk(grid, *c)
                };
                if mb.indices.is_empty() {
                    self.voxel_meshes
                        .remove(&(crate::gfx::DestructibleId(0), c.x, c.y, c.z));
                    // Also drop any stale chunk collider so debris-vs-world avoids dead volumes
                    self.chunk_colliders.retain(|sc| sc.coord != *c);
                    // Evict cached hash so future solidification can't be skipped
                    self.voxel_hashes.remove(&key);
                } else {
                    // Interleave positions + normals to match types::Vertex layout
                    let mesh_cpu = ecs_core::components::MeshCpu {
                        positions: mb.positions.clone(),
                        normals: mb.normals.clone(),
                        indices: mb.indices.clone(),
                    };
                    let _ = crate::gfx::renderer::voxel_upload::upload_chunk_mesh(
                        &self.device,
                        crate::gfx::DestructibleId(0),
                        (c.x, c.y, c.z),
                        &mesh_cpu,
                        &mut self.voxel_meshes,
                        &mut self.voxel_hashes,
                    );
                    // Preserve occupancy-hash skip optimization
                    self.voxel_hashes.insert(key, h);
                }
            }
            self.vox_skipped_last = skipped;
            self.vox_remesh_ms_last = t0.elapsed().as_secs_f32() * 1000.0;
            // Refresh coarse colliders for these chunks
            if self.destruct_cfg.debris_vs_world {
                let t1 = Instant::now();
                let mut updates: Vec<collision_static::chunks::StaticChunk> = Vec::new();
                for c in &chunks {
                    if let Some(col) = chunkcol::build_chunk_collider(grid, *c) {
                        updates.push(col);
                    }
                }
                if !updates.is_empty() {
                    chunkcol::swap_in_updates(&mut self.chunk_colliders, updates);
                    self.static_index = Some(chunkcol::rebuild_static_index(&self.chunk_colliders));
                }
                self.vox_collider_ms_last = t1.elapsed().as_secs_f32() * 1000.0;
            }
        }
        self.vox_last_chunks = chunks.len();
        self.vox_queue_len = self.chunk_queue.len();
    }

    pub(crate) fn update_debris(&mut self, dt: f32) {
        if self.debris.is_empty() {
            self.debris_count = 0;
            return;
        }
        let g = glam::Vec3::new(0.0, -9.8, 0.0);
        let mut instances: Vec<Instance> = Vec::with_capacity(self.debris.len());
        let vsize = self.destruct_cfg.voxel_size_m.0 as f32;
        let half = vsize * 0.5;
        let mut i = 0usize;
        while i < self.debris.len() {
            let d = &mut self.debris[i];
            d.vel += g * dt;
            d.pos += d.vel * dt;
            // Ground collision
            let (h, _n) = crate::gfx::terrain::height_at(&self.terrain_cpu, d.pos.x, d.pos.z);
            let floor = h + half;
            if d.pos.y < floor {
                d.pos.y = floor;
                d.vel.y = -d.vel.y * 0.35;
                d.vel.x *= 0.98;
                d.vel.z *= 0.98;
            }
            d.age += dt;
            if d.age > d.life {
                self.debris.swap_remove(i);
                continue;
            }
            let m = glam::Mat4::from_scale_rotation_translation(
                glam::Vec3::splat(vsize),
                glam::Quat::IDENTITY,
                d.pos,
            );
            instances.push(Instance {
                model: m.to_cols_array_2d(),
                color: [0.55, 0.55, 0.55],
                selected: 0.0,
            });
            i += 1;
        }
        self.debris_count = instances.len() as u32;
        if self.debris_count > 0 {
            let bytes: &[u8] = bytemuck::cast_slice(&instances);
            self.queue.write_buffer(&self.debris_instances, 0, bytes);
        }
    }

    pub(crate) fn spawn_firebolt(
        &mut self,
        origin: glam::Vec3,
        dir: glam::Vec3,
        t: f32,
        owner: Option<usize>,
        snap_to_ground: bool,
        color: [f32; 3],
    ) {
        let mut speed = 40.0;
        // Base lifetime for visuals; will be clamped by spec range below.
        let base_life = 1.2 * 1.5;
        // Compute range clamp from spell spec (default 120 ft)
        let mut max_range_m = 120.0 * 0.3048;
        if let Some(spec) = &self.fire_bolt
            && let Some(p) = &spec.projectile
        {
            speed = p.speed_mps;
            max_range_m = (spec.range_ft as f32) * 0.3048;
        }
        let flight_time = if speed > 0.01 {
            max_range_m / speed
        } else {
            base_life
        };
        let life = base_life.min(flight_time);
        // Ensure initial spawn is terrain-aware.
        let origin = if snap_to_ground {
            let (h, _n) = terrain::height_at(&self.terrain_cpu, origin.x, origin.z);
            glam::vec3(origin.x, h + 0.15, origin.z)
        } else {
            gfx::util::clamp_above_terrain(&self.terrain_cpu, origin, 0.15)
        };
        self.projectiles.push(gfx::fx::Projectile {
            pos: origin,
            vel: dir * speed,
            t_die: t + life,
            owner_wizard: owner,
            color,
            kind: crate::gfx::fx::ProjectileKind::Normal,
        });
    }

    /// Spawn Magic Missile visuals: three darts on a horizontal plane.
    /// The center dart flies straight forward; the side darts fly with a slight
    /// outward yaw so they gradually spread as they travel.
    pub(crate) fn spawn_magic_missile(&mut self, origin: glam::Vec3, dir: glam::Vec3, t: f32) {
        let base_dir = dir.normalize_or_zero();
        // Ultra-tight spread: ±2 degrees about Y (horizontal plane)
        let spread_rad = 2.0_f32.to_radians();
        let left_dir = glam::Quat::from_rotation_y(-spread_rad) * base_dir;
        let right_dir = glam::Quat::from_rotation_y(spread_rad) * base_dir;

        let mm_col = [1.3, 0.7, 2.3];
        // Spawn all three at the same origin so they separate over distance
        self.spawn_firebolt(origin, base_dir, t, Some(self.pc_index), false, mm_col);
        self.spawn_firebolt(origin, left_dir, t, Some(self.pc_index), false, mm_col);
        self.spawn_firebolt(origin, right_dir, t, Some(self.pc_index), false, mm_col);
    }

    pub(crate) fn spawn_fireball(
        &mut self,
        origin: glam::Vec3,
        dir: glam::Vec3,
        t: f32,
        owner: Option<usize>,
    ) {
        let speed = 28.0f32; // slower, chunky orb
        let base_life = 2.0f32; // seconds max
        // Fireball SRD: 150 ft range. Use that for flight clamp if we later aim.
        let max_range_m = 150.0f32 * 0.3048;
        let flight_time = max_range_m / speed;
        let life = base_life.min(flight_time);
        let origin = gfx::util::clamp_above_terrain(&self.terrain_cpu, origin, 0.15);
        self.projectiles.push(gfx::fx::Projectile {
            pos: origin,
            vel: dir.normalize_or_zero() * speed,
            t_die: t + life,
            owner_wizard: owner,
            color: [2.2, 0.7, 0.2],
            kind: crate::gfx::fx::ProjectileKind::Fireball {
                radius: 6.0,
                damage: 28, // avg 8d6; prototype without saves
            },
        });
    }

    fn explode_fireball_at(
        &mut self,
        owner: Option<usize>,
        center: glam::Vec3,
        radius: f32,
        damage: i32,
    ) {
        // Visual explosion burst
        for _ in 0..42 {
            let a = rand_unit() * std::f32::consts::TAU;
            let r = 6.0 + rand_unit() * 2.0;
            self.particles.push(Particle {
                pos: center,
                vel: glam::vec3(a.cos() * r, 3.0 + rand_unit() * 2.0, a.sin() * r),
                age: 0.0,
                life: 0.28,
                size: 0.05,
                color: [2.2, 1.0, 0.3],
            });
        }
        // Damage NPCs in radius
        let r2 = radius * radius;
        // Handle DK first for despawn behavior
        if let Some(dk_id) = self.dk_id
            && let Some(n) = self.server.npcs.iter_mut().find(|n| n.id == dk_id)
            && n.alive
        {
            let dx = n.pos.x - center.x;
            let dz = n.pos.z - center.z;
            if dx * dx + dz * dz <= r2 {
                let before = n.hp;
                n.hp = (n.hp - damage).max(0);
                let fatal = n.hp == 0;
                if fatal {
                    n.alive = false;
                    self.dk_count = 0;
                    self.dk_id = None;
                }
                let (hgt, _n) = crate::gfx::terrain::height_at(&self.terrain_cpu, n.pos.x, n.pos.z);
                self.damage
                    .spawn(glam::vec3(n.pos.x, hgt + n.radius + 0.9, n.pos.z), damage);
                let _ = before; // reserved for future events
            }
        }
        // Destructible: ruins -> voxelize on first impact, then carve (one‑path demo only)
        #[cfg(feature = "vox_onepath_demo")]
        if self.vox_onepath_ui.is_some() {
            // If we already have a voxel grid (from a prior impact), carve it.
            if let Some(grid) = self.voxel_grid.as_mut() {
                let _out = carve_and_spawn_debris(
                    grid,
                    glam::DVec3::new(center.x as f64, center.y as f64, center.z as f64),
                    core_units::Length::meters((radius * 0.25) as f64),
                    self.destruct_cfg.seed,
                    self.impact_id,
                    self.destruct_cfg.max_debris,
                );
                self.impact_id = self.impact_id.wrapping_add(1);
                let enq = grid.pop_dirty_chunks(usize::MAX);
                self.chunk_queue.enqueue_many(enq);
                self.vox_queue_len = self.chunk_queue.len();
                // Process some chunks so geometry updates promptly
                let saved = self.destruct_cfg.max_chunk_remesh;
                self.destruct_cfg.max_chunk_remesh = 32;
                self.process_voxel_queues();
                self.destruct_cfg.max_chunk_remesh = saved;
            } else {
                // Find nearest ruins instance
                if !self.ruins_instances_cpu.is_empty() {
                    let mut best = (usize::MAX, f32::INFINITY, glam::Vec3::ZERO);
                    for (i, inst) in self.ruins_instances_cpu.iter().enumerate() {
                        let m = glam::Mat4::from_cols_array_2d(&inst.model);
                        let pos = m.transform_point3(glam::Vec3::ZERO);
                        let d2 = pos.distance_squared(center);
                        if d2 < best.1 {
                            best = (i, d2, pos);
                        }
                    }
                    let idx = best.0;
                    if idx != usize::MAX {
                        let pos = best.2;
                        let horiz =
                            glam::vec2(pos.x, pos.z).distance(glam::vec2(center.x, center.z));
                        let approx_ruins_radius = 6.5f32; // horizontal radius estimate
                        if horiz < approx_ruins_radius * 1.6 {
                            // Hide instance and build voxel grid around it (approximate box)
                            let half = glam::vec3(
                                approx_ruins_radius,
                                approx_ruins_radius * 0.8,
                                approx_ruins_radius,
                            );
                            self.hide_ruins_instance(idx);
                            self.build_voxel_grid_for_ruins(pos, half);
                            if let Some(grid) = self.voxel_grid.as_mut() {
                                let _ = carve_and_spawn_debris(
                                    grid,
                                    glam::DVec3::new(
                                        center.x as f64,
                                        center.y as f64,
                                        center.z as f64,
                                    ),
                                    core_units::Length::meters((radius * 0.25) as f64),
                                    self.destruct_cfg.seed,
                                    self.impact_id,
                                    self.destruct_cfg.max_debris,
                                );
                                self.impact_id = self.impact_id.wrapping_add(1);
                                let enq = grid.pop_dirty_chunks(usize::MAX);
                                self.chunk_queue.enqueue_many(enq);
                                self.vox_queue_len = self.chunk_queue.len();
                                let saved = self.destruct_cfg.max_chunk_remesh;
                                self.destruct_cfg.max_chunk_remesh = 64;
                                while self.vox_queue_len > 0 {
                                    self.process_voxel_queues();
                                }
                                self.destruct_cfg.max_chunk_remesh = saved;
                            }
                        }
                    }
                }
            }
        }
        // Generic NPCs + zombies
        let mut k = 0usize;
        while k < self.server.npcs.len() {
            let id = self.server.npcs[k].id;
            if !self.server.npcs[k].alive {
                k += 1;
                continue;
            }
            let dx = self.server.npcs[k].pos.x - center.x;
            let dz = self.server.npcs[k].pos.z - center.z;
            if dx * dx + dz * dz <= r2 {
                let before = self.server.npcs[k].hp;
                self.server.npcs[k].hp = (self.server.npcs[k].hp - damage).max(0);
                let fatal = self.server.npcs[k].hp == 0;
                if fatal {
                    self.server.npcs[k].alive = false;
                }
                // UI floater
                if let Some(idx) = self.zombie_ids.iter().position(|nid| *nid == id) {
                    // Spawn above zombie head using its model
                    let m = self
                        .zombie_models
                        .get(idx)
                        .copied()
                        .unwrap_or(glam::Mat4::IDENTITY);
                    let head = m * glam::Vec4::new(0.0, 1.6, 0.0, 1.0);
                    self.damage.spawn(head.truncate(), damage);
                    if fatal {
                        self.zombie_ids.swap_remove(idx);
                        self.zombie_models.swap_remove(idx);
                        if (idx as u32) < self.zombie_count {
                            self.zombie_instances_cpu.swap_remove(idx);
                            self.zombie_count -= 1;
                            for (i, inst) in self.zombie_instances_cpu.iter_mut().enumerate() {
                                inst.palette_base = (i as u32) * self.zombie_joints;
                            }
                            let bytes: &[u8] = bytemuck::cast_slice(&self.zombie_instances_cpu);
                            self.queue.write_buffer(&self.zombie_instances, 0, bytes);
                        }
                    }
                } else {
                    let (hgt, _n) = crate::gfx::terrain::height_at(
                        &self.terrain_cpu,
                        self.server.npcs[k].pos.x,
                        self.server.npcs[k].pos.z,
                    );
                    self.damage.spawn(
                        glam::vec3(
                            self.server.npcs[k].pos.x,
                            hgt + self.server.npcs[k].radius + 0.9,
                            self.server.npcs[k].pos.z,
                        ),
                        damage,
                    );
                }
                let _ = before;
            }
            k += 1;
        }
        // Damage wizards (including PC) in radius; trigger aggro if player-owned explosion hits any wizard
        let mut hit_any_wizard = false;
        let mut to_remove: Vec<usize> = Vec::new();
        for j in 0..(self.wizard_count as usize) {
            let hp = self.wizard_hp.get(j).copied().unwrap_or(self.wizard_hp_max);
            if hp <= 0 {
                continue;
            }
            let c = self.wizard_models[j].to_cols_array();
            let pos = glam::vec3(c[12], c[13], c[14]);
            let dx = pos.x - center.x;
            let dz = pos.z - center.z;
            if dx * dx + dz * dz <= r2 {
                let before = self.wizard_hp[j];
                let after = (before - damage).max(0);
                self.wizard_hp[j] = after;
                hit_any_wizard = hit_any_wizard || owner == Some(self.pc_index);
                let head = pos + glam::vec3(0.0, 1.7, 0.0);
                self.damage.spawn(head, damage);
                if after == 0 {
                    if j == self.pc_index {
                        self.kill_pc();
                    } else {
                        to_remove.push(j);
                    }
                }
            }
        }
        // Remove dead wizards after the loop (descending indices to preserve validity)
        if !to_remove.is_empty() {
            to_remove.sort_unstable_by(|a, b| b.cmp(a));
            for idx in to_remove {
                if idx < self.wizard_count as usize {
                    self.remove_wizard_at(idx);
                }
            }
        }
        if hit_any_wizard {
            self.wizards_hostile_to_pc = true;
            // Ensure NPC wizards resume casting loop even if all monsters are dead
            for i in 0..(self.wizard_count as usize) {
                if i == self.pc_index {
                    continue;
                }
                if self.wizard_hp.get(i).copied().unwrap_or(0) <= 0 {
                    continue;
                }
                if self.wizard_anim_index[i] != 0 {
                    self.wizard_anim_index[i] = 0;
                    self.wizard_last_phase[i] = 0.0;
                }
            }
        }
    }

    pub(crate) fn right_hand_world(&self, clip: &AnimClip, phase: f32) -> Option<glam::Vec3> {
        let h = self.hand_right_node?;
        let m = anim::global_of_node(&self.skinned_cpu, clip, phase, h)?;
        let c = m.to_cols_array();
        Some(glam::vec3(c[12], c[13], c[14]))
    }

    #[allow(dead_code)]
    pub(crate) fn root_flat_forward(&self, clip: &AnimClip, phase: f32) -> Option<glam::Vec3> {
        let r = self.root_node?;
        let m = anim::global_of_node(&self.skinned_cpu, clip, phase, r)?;
        let z = (m * glam::Vec4::new(0.0, 0.0, 1.0, 0.0)).truncate();
        let mut f = z;
        f.y = 0.0;
        if f.length_squared() > 1e-6 {
            Some(f.normalize())
        } else {
            None
        }
    }
}

// No-op stubs for demo helpers when vox_onepath_demo is disabled
#[cfg(not(feature = "vox_onepath_demo"))]
impl Renderer {
    #[allow(dead_code)]
    pub fn process_voxel_queues(&mut self) {}
    #[allow(dead_code)]
    fn build_voxel_grid_for_ruins(&mut self, _center: glam::Vec3, _half_extent: glam::Vec3) {}
    #[allow(dead_code)]
    pub(crate) fn reset_voxel_and_replay(&mut self) {}
    #[allow(dead_code)]
    fn seed_voxel_chunk_colliders(&mut self, _grid: &voxel_proxy::VoxelGrid) {}
}

#[cfg(not(feature = "legacy_client_carve"))]
impl Renderer {
    pub(crate) fn try_voxel_impact(&mut self, _p0: glam::Vec3, _p1: glam::Vec3) {}
}

// Small helpers used by input/update
pub(super) fn wrap_angle(a: f32) -> f32 {
    let mut x = a;
    while x > std::f32::consts::PI {
        x -= std::f32::consts::TAU;
    }
    while x < -std::f32::consts::PI {
        x += std::f32::consts::TAU;
    }
    x
}

impl Renderer {
    #[cfg(feature = "vox_onepath_demo")]
    pub(crate) fn reset_voxel_and_replay(&mut self) {
        // Reset grid to initial state if available
        let initial = self.voxel_grid_initial.clone();
        if let Some(init) = initial {
            self.voxel_grid = Some(init);
            self.impact_id = 0;
            self.voxel_meshes.clear();
            self.voxel_hashes.clear();
            self.chunk_colliders.clear();
            self.static_index = None;
            // Enqueue all chunks
            if let Some(ref grid) = self.voxel_grid {
                let dims = grid.dims();
                let csz = grid.meta().chunk;
                let nx = dims.x.div_ceil(csz.x);
                let ny = dims.y.div_ceil(csz.y);
                let nz = dims.z.div_ceil(csz.z);
                for cz in 0..nz {
                    for cy in 0..ny {
                        for cx in 0..nx {
                            self.chunk_queue
                                .enqueue_many([glam::UVec3::new(cx, cy, cz)]);
                        }
                    }
                }
                self.vox_queue_len = self.chunk_queue.len();
            }
            // Clear debris
            self.debris.clear();
            self.debris_count = 0;
            // Replay recent impacts deterministically
            let rec = self.recent_impacts.clone();
            if let Some(grid) = self.voxel_grid.as_mut() {
                for (center, r) in rec {
                    let _ = server_core::destructible::carve_and_spawn_debris(
                        grid,
                        center,
                        core_units::Length::meters(r),
                        self.destruct_cfg.seed,
                        self.impact_id,
                        self.destruct_cfg.max_debris,
                    );
                    self.impact_id = self.impact_id.wrapping_add(1);
                    let enq = grid.pop_dirty_chunks(usize::MAX);
                    self.chunk_queue.enqueue_many(enq);
                }
            }
            destruct_log!(
                "Voxel world reset; replayed {} impacts",
                self.recent_impacts.len()
            );
        }
    }
}

pub(super) fn rand_unit() -> f32 {
    use rand::Rng as _;
    let mut r = rand::rng();
    r.random::<f32>() * 2.0 - 1.0
}

pub(super) fn segment_hits_circle_xz(
    p0: glam::Vec3,
    p1: glam::Vec3,
    c: glam::Vec3,
    r: f32,
) -> bool {
    let p0 = glam::vec2(p0.x, p0.z);
    let p1 = glam::vec2(p1.x, p1.z);
    let c = glam::vec2(c.x, c.z);
    let d = p1 - p0;
    let m = p0 - c;
    let a = d.dot(d);
    if a <= 1e-6 {
        return m.length() <= r;
    }
    let t = (-(m.dot(d)) / a).clamp(0.0, 1.0);
    let closest = p0 + d * t;
    (closest - c).length() <= r
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn segment_circle_intersects_center_cross() {
        let c = glam::vec3(0.0, 0.0, 0.0);
        let p0 = glam::vec3(-2.0, 0.5, 0.0);
        let p1 = glam::vec3(2.0, 0.5, 0.0);
        assert!(segment_hits_circle_xz(p0, p1, c, 0.5));
    }

    #[test]
    fn ray_box_hits_from_outside() {
        // Ray along +X hits a unit AABB centered at origin
        let p0 = glam::vec3(-10.0, 0.5, 0.0);
        let dir = glam::vec3(1.0, 0.0, 0.0);
        let bmin = glam::vec3(-1.0, -1.0, -1.0);
        let bmax = glam::vec3(1.0, 1.0, 1.0);

        let (t_enter, t_exit) = Renderer::ray_box_intersect(p0, dir, bmin, bmax).expect("hit");
        assert!(t_enter > 0.0, "enter should be ahead of origin");
        assert!(t_exit > t_enter, "exit should be after enter");

        let hit = p0 + dir * t_enter;
        // Inside AABB with small tolerance
        assert!(hit.x >= bmin.x - 1e-4 && hit.x <= bmax.x + 1e-4);
        assert!(hit.y >= bmin.y - 1e-4 && hit.y <= bmax.y + 1e-4);
        assert!(hit.z >= bmin.z - 1e-4 && hit.z <= bmax.z + 1e-4);
    }

    #[test]
    fn ray_box_parallel_miss() {
        // Ray is parallel to X axis and AABB is out of its Y slab -> miss
        let p0 = glam::vec3(-10.0, 2.1, 0.0);
        let dir = glam::vec3(1.0, 0.0, 0.0);
        let bmin = glam::vec3(-1.0, -1.0, -1.0);
        let bmax = glam::vec3(1.0, 1.0, 1.0);

        assert!(Renderer::ray_box_intersect(p0, dir, bmin, bmax).is_none());
    }

    #[test]
    fn ray_box_starting_inside() {
        // Starting inside should clamp t_enter to 0
        let p0 = glam::vec3(0.0, 0.0, 0.0);
        let dir = glam::vec3(1.0, 0.0, 0.0);
        let bmin = glam::vec3(-1.0, -1.0, -1.0);
        let bmax = glam::vec3(1.0, 1.0, 1.0);

        let (t_enter, t_exit) = Renderer::ray_box_intersect(p0, dir, bmin, bmax).expect("hit");
        assert!(t_enter.abs() < 1e-6, "starts inside => t_enter ~= 0");
        assert!(t_exit > 0.0);
    }

    #[test]
    fn segment_circle_hits_cases() {
        let c = glam::vec3(0.0, 0.0, 0.0);
        // Cross the center
        assert!(segment_hits_circle_xz(
            glam::vec3(-2.0, 0.0, 0.0),
            glam::vec3(2.0, 0.0, 0.0),
            c,
            0.5
        ));
        // Graze just inside
        assert!(segment_hits_circle_xz(
            glam::vec3(-1.0, 0.0, 0.49),
            glam::vec3(1.0, 0.0, 0.49),
            c,
            0.5
        ));
        // Miss just outside
        assert!(!segment_hits_circle_xz(
            glam::vec3(-1.0, 0.0, 0.6),
            glam::vec3(1.0, 0.0, 0.6),
            c,
            0.5
        ));
    }

    #[test]
    fn destruct_log_compiles() {
        // Should compile in both modes; no logger required in tests.
        destruct_log!("destructible test {}", 42);
    }
}
