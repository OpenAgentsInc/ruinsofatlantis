//! Animation retargeting between humanoid rigs.

use anyhow::{Result, bail};
use glam::{Mat4, Quat, Vec3};
use std::collections::HashMap;

use crate::humanoid::{HumanoidBone, HumanoidRig, detect_humanoid};
use crate::types::{AnimClip, SkinnedMeshCPU, TrackQuat, TrackVec3};

#[derive(Clone, Debug)]
pub struct RetargetOptions {
    pub preserve_root_motion: bool,
    pub apply_rest_correction: bool,
    pub scale_override: Option<f32>,
    pub allowlist: Option<Vec<String>>,
}

pub fn retarget_animations(
    src: &SkinnedMeshCPU,
    dst: &mut SkinnedMeshCPU,
    opts: &RetargetOptions,
) -> Result<()> {
    let rig_s = detect_humanoid(src);
    let rig_d = detect_humanoid(dst);

    let mut map: Vec<(usize, usize, HumanoidBone)> = Vec::new();
    for b in 0..HumanoidBone::COUNT {
        if let (Some(ns), Some(nd)) = (rig_s.node_of_bone[b], rig_d.node_of_bone[b]) {
            let hb = match b {
                0 => HumanoidBone::Hips,
                1 => HumanoidBone::Spine,
                2 => HumanoidBone::Chest,
                3 => HumanoidBone::UpperChest,
                4 => HumanoidBone::Neck,
                5 => HumanoidBone::Head,
                6 => HumanoidBone::ClavicleL,
                7 => HumanoidBone::UpperArmL,
                8 => HumanoidBone::LowerArmL,
                9 => HumanoidBone::HandL,
                10 => HumanoidBone::ClavicleR,
                11 => HumanoidBone::UpperArmR,
                12 => HumanoidBone::LowerArmR,
                13 => HumanoidBone::HandR,
                14 => HumanoidBone::UpperLegL,
                15 => HumanoidBone::LowerLegL,
                16 => HumanoidBone::FootL,
                17 => HumanoidBone::ToesL,
                18 => HumanoidBone::UpperLegR,
                19 => HumanoidBone::LowerLegR,
                20 => HumanoidBone::FootR,
                21 => HumanoidBone::ToesR,
                _ => continue,
            };
            map.push((ns, nd, hb));
        }
    }
    if map.is_empty() {
        bail!("retarget: no overlapping humanoid bones");
    }

    let scale = if let Some(s) = opts.scale_override {
        s
    } else {
        fn seg_len(sk: &SkinnedMeshCPU, rig: &HumanoidRig, b: HumanoidBone) -> f32 {
            rig.node_of_bone[b as usize]
                .and_then(|i| sk.base_t.get(i).copied())
                .unwrap_or(Vec3::ZERO)
                .length()
        }
        let s_leg = seg_len(src, &rig_s, HumanoidBone::UpperLegL)
            + seg_len(src, &rig_s, HumanoidBone::LowerLegL)
            + seg_len(src, &rig_s, HumanoidBone::FootL);
        let d_leg = seg_len(dst, &rig_d, HumanoidBone::UpperLegL)
            + seg_len(dst, &rig_d, HumanoidBone::LowerLegL)
            + seg_len(dst, &rig_d, HumanoidBone::FootL);
        if d_leg <= 0.0 {
            1.0
        } else {
            (s_leg / d_leg).clamp(0.25, 4.0)
        }
    };

    for (name, clip) in &src.animations {
        if let Some(allow) = &opts.allowlist
            && !allow.iter().any(|a| a == name)
        {
            continue;
        }
        let mut out = AnimClip {
            name: name.clone(),
            duration: clip.duration,
            t_tracks: HashMap::new(),
            r_tracks: HashMap::new(),
            s_tracks: HashMap::new(),
        };
        for (ns, nd, _hb) in &map {
            let nt_s = clip.t_tracks.get(ns).cloned().unwrap_or(TrackVec3 {
                times: vec![],
                values: vec![],
            });
            let nr_s = clip.r_tracks.get(ns).cloned().unwrap_or(TrackQuat {
                times: vec![],
                values: vec![],
            });
            let ns_s = clip.s_tracks.get(ns).cloned().unwrap_or(TrackVec3 {
                times: vec![],
                values: vec![],
            });
            let rest_s = local_rest(src, *ns);
            let rest_d = local_rest(dst, *nd);
            let corr = if opts.apply_rest_correction {
                rest_d * rest_s.inverse()
            } else {
                Mat4::IDENTITY
            };
            let times = union_times(&nr_s.times, &nt_s.times, &ns_s.times);
            if times.is_empty() {
                continue;
            }
            let mut t_vals = Vec::with_capacity(times.len());
            let mut r_vals = Vec::with_capacity(times.len());
            let mut s_vals = Vec::with_capacity(times.len());
            for &t in &times {
                let t_s = sample_vec3(&nt_s, t).unwrap_or(src.base_t[*ns]);
                let r_s = sample_quat(&nr_s, t).unwrap_or(src.base_r[*ns]);
                let s_s = sample_vec3(&ns_s, t).unwrap_or(src.base_s[*ns]);
                let mut m = Mat4::from_scale_rotation_translation(s_s, r_s, t_s);
                m = corr * m;
                let (t_l, r_l, mut s_l) = decompose(m);
                s_l *= scale;
                t_vals.push(t_l);
                r_vals.push(r_l.normalize());
                s_vals.push(s_l);
            }
            out.t_tracks.insert(
                *nd,
                TrackVec3 {
                    times: times.clone(),
                    values: t_vals,
                },
            );
            out.r_tracks.insert(
                *nd,
                TrackQuat {
                    times: times.clone(),
                    values: r_vals,
                },
            );
            out.s_tracks.insert(
                *nd,
                TrackVec3 {
                    times,
                    values: s_vals,
                },
            );
        }
        if opts.preserve_root_motion
            && let (Some(ns_root), Some(nd_root)) = (
                rig_s.node_of_bone[HumanoidBone::Hips as usize],
                rig_d.node_of_bone[HumanoidBone::Hips as usize],
            )
        {
            if let Some(src_t) = clip.t_tracks.get(&ns_root).cloned() {
                let mut scaled = src_t.clone();
                for v in &mut scaled.values {
                    *v *= scale;
                }
                out.t_tracks.insert(nd_root, scaled);
            }
            if let Some(src_r) = clip.r_tracks.get(&ns_root).cloned() {
                out.r_tracks.insert(nd_root, src_r);
            }
        }
        dst.animations.insert(out.name.clone(), out);
    }
    Ok(())
}

fn local_rest(sk: &SkinnedMeshCPU, n: usize) -> Mat4 {
    Mat4::from_scale_rotation_translation(sk.base_s[n], sk.base_r[n], sk.base_t[n])
}

fn union_times(a: &[f32], b: &[f32], c: &[f32]) -> Vec<f32> {
    use std::collections::BTreeSet;
    let mut set = BTreeSet::new();
    for &t in a.iter().chain(b).chain(c) {
        set.insert((t * 1000.0).round() as i32);
    }
    set.into_iter().map(|q| (q as f32) / 1000.0).collect()
}

fn sample_vec3(tr: &TrackVec3, t: f32) -> Option<Vec3> {
    if tr.times.is_empty() {
        return None;
    }
    let mut last = 0usize;
    for (i, &tt) in tr.times.iter().enumerate() {
        if tt > t {
            break;
        }
        last = i;
    }
    tr.values.get(last).copied()
}
fn sample_quat(tr: &TrackQuat, t: f32) -> Option<Quat> {
    if tr.times.is_empty() {
        return None;
    }
    let mut last = 0usize;
    for (i, &tt) in tr.times.iter().enumerate() {
        if tt > t {
            break;
        }
        last = i;
    }
    tr.values.get(last).copied()
}
fn decompose(m: Mat4) -> (Vec3, Quat, Vec3) {
    let (s, r, t) = m.to_scale_rotation_translation();
    (t, r, s)
}
