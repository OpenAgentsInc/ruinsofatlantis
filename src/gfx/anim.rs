//! Animation sampling helpers for skinned meshes.
//!
//! CPU-side sampling of glTF animation tracks to joint palettes, plus small
//! query helpers used by the renderer.

use crate::assets::{AnimClip, SkinnedMeshCPU, TrackQuat, TrackVec3};

pub fn sample_palette(mesh: &SkinnedMeshCPU, clip: &AnimClip, t: f32) -> Vec<glam::Mat4> {
    use std::collections::HashMap;
    let mut local_t: Vec<glam::Vec3> = mesh.base_t.clone();
    let mut local_r: Vec<glam::Quat> = mesh.base_r.clone();
    let mut local_s: Vec<glam::Vec3> = mesh.base_s.clone();

    let time = if clip.duration > 0.0 {
        t % clip.duration
    } else {
        0.0
    };

    // Apply tracks to local TRS
    for (node, tr) in &clip.t_tracks {
        local_t[*node] = sample_vec3(tr, time, mesh.base_t[*node]);
    }
    for (node, rr) in &clip.r_tracks {
        local_r[*node] = sample_quat(rr, time, mesh.base_r[*node]);
    }
    for (node, sr) in &clip.s_tracks {
        local_s[*node] = sample_vec3(sr, time, mesh.base_s[*node]);
    }

    // Compute global matrices for all nodes touched by joints
    let mut global: HashMap<usize, glam::Mat4> = HashMap::new();
    for &jn in &mesh.joints_nodes {
        if jn < local_t.len() {
            compute_global(jn, &mesh.parent, &local_t, &local_r, &local_s, &mut global);
        }
    }

    // Build palette: global * inverse_bind per joint in skin order
    let mut out = Vec::with_capacity(mesh.joints_nodes.len());
    for (i, &node_idx) in mesh.joints_nodes.iter().enumerate() {
        let g = if node_idx < local_t.len() {
            *global.get(&node_idx).unwrap_or(&glam::Mat4::IDENTITY)
        } else {
            glam::Mat4::IDENTITY
        };
        let ibm = mesh
            .inverse_bind
            .get(i)
            .copied()
            .unwrap_or(glam::Mat4::IDENTITY);
        out.push(g * ibm);
    }
    out
}

pub fn global_of_node(
    mesh: &SkinnedMeshCPU,
    clip: &AnimClip,
    t: f32,
    node_idx: usize,
) -> Option<glam::Mat4> {
    let mut lt = mesh.base_t.clone();
    let mut lr = mesh.base_r.clone();
    let mut ls = mesh.base_s.clone();
    let time = if clip.duration > 0.0 {
        t % clip.duration
    } else {
        0.0
    };
    if let Some(tr) = clip.t_tracks.get(&node_idx) {
        lt[node_idx] = sample_vec3(tr, time, lt[node_idx]);
    }
    if let Some(rr) = clip.r_tracks.get(&node_idx) {
        lr[node_idx] = sample_quat(rr, time, lr[node_idx]);
    }
    if let Some(sr) = clip.s_tracks.get(&node_idx) {
        ls[node_idx] = sample_vec3(sr, time, ls[node_idx]);
    }
    let mut cache = std::collections::HashMap::new();
    Some(compute_global(
        node_idx,
        &mesh.parent,
        &lt,
        &lr,
        &ls,
        &mut cache,
    ))
}

pub fn compute_portalopen_strikes(
    mesh: &SkinnedMeshCPU,
    hand_right_node: Option<usize>,
    _root_node: Option<usize>,
) -> Vec<f32> {
    if let (Some(hand), Some(clip)) = (hand_right_node, mesh.animations.get("PortalOpen")) {
        if let Some(trk) = clip.t_tracks.get(&hand) {
            if trk.times.len() >= 3 {
                let mut min_y = f32::INFINITY;
                for v in &trk.values {
                    if v.y < min_y {
                        min_y = v.y;
                    }
                }
                let thresh = min_y + 0.02;
                let mut out = Vec::new();
                for i in 1..(trk.times.len() - 1) {
                    let y0 = trk.values[i - 1].y;
                    let y1 = trk.values[i].y;
                    let y2 = trk.values[i + 1].y;
                    if y1 < y0 && y1 < y2 && y1 <= thresh {
                        out.push(trk.times[i]);
                    }
                }
                if !out.is_empty() {
                    return out;
                }
            }
        }
        // Fallback: periodic triggers if hand track missing
        if clip.duration > 0.0 {
            let mut out = Vec::new();
            let mut t = clip.duration * 0.25;
            while t < clip.duration {
                out.push(t);
                t += 0.9;
            }
            return out;
        }
    }
    Vec::new()
}

fn compute_global(
    node: usize,
    parent: &Vec<Option<usize>>,
    lt: &Vec<glam::Vec3>,
    lr: &Vec<glam::Quat>,
    ls: &Vec<glam::Vec3>,
    cache: &mut std::collections::HashMap<usize, glam::Mat4>,
) -> glam::Mat4 {
    if let Some(m) = cache.get(&node) {
        return *m;
    }
    let local = glam::Mat4::from_scale_rotation_translation(ls[node], lr[node], lt[node]);
    let m = if let Some(p) = parent[node] {
        compute_global(p, parent, lt, lr, ls, cache) * local
    } else {
        local
    };
    cache.insert(node, m);
    m
}

fn sample_vec3(tr: &TrackVec3, t: f32, default: glam::Vec3) -> glam::Vec3 {
    if tr.times.is_empty() {
        return default;
    }
    if t <= tr.times[0] {
        return tr.values[0];
    }
    if t >= *tr.times.last().unwrap() {
        return *tr.values.last().unwrap();
    }
    let mut i = 0;
    while i + 1 < tr.times.len() && !(t >= tr.times[i] && t <= tr.times[i + 1]) {
        i += 1;
    }
    let t0 = tr.times[i];
    let t1 = tr.times[i + 1];
    let f = (t - t0) / (t1 - t0);
    tr.values[i].lerp(tr.values[i + 1], f)
}

fn sample_quat(tr: &TrackQuat, t: f32, default: glam::Quat) -> glam::Quat {
    if tr.times.is_empty() {
        return default;
    }
    if t <= tr.times[0] {
        return tr.values[0];
    }
    if t >= *tr.times.last().unwrap() {
        return *tr.values.last().unwrap();
    }
    let mut i = 0;
    while i + 1 < tr.times.len() && !(t >= tr.times[i] && t <= tr.times[i + 1]) {
        i += 1;
    }
    let t0 = tr.times[i];
    let t1 = tr.times[i + 1];
    let f = (t - t0) / (t1 - t0);
    tr.values[i].slerp(tr.values[i + 1], f)
}
