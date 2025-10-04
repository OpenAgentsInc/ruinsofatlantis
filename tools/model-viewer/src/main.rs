use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use glam::{Mat4, Vec3};
use log::info;
use ra_assets::skinning::{merge_fbx_animations, merge_gltf_animations};
use ra_assets::{CpuMesh, SkinnedMeshCPU, load_gltf_mesh, load_gltf_skinned};
use wgpu::util::DeviceExt;
use wgpu::{SurfaceTargetUnsafe, rwh::HasDisplayHandle, rwh::HasWindowHandle};
use winit::{dpi::PhysicalSize, event::*, event_loop::EventLoop, window::WindowAttributes};

// 5x7 bitmap rows for ASCII A-Z, 0-9, space, colon, underscore, hyphen
fn glyph5x7_rows(c: char) -> [u8; 7] {
    match c {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01111, 0b10000, 0b10000, 0b10011, 0b10001, 0b10001, 0b01110,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        'J' => [
            0b00001, 0b00001, 0b00001, 0b00001, 0b10001, 0b10001, 0b01110,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b01010, 0b01010, 0b00100, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b11011, 0b10001,
        ],
        'X' => [
            0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00010, 0b00100, 0b01000, 0b10000, 0b10000, 0b11111,
        ],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ],
        '6' => [
            0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ],
        ':' => [
            0b00000, 0b00100, 0b00000, 0b00000, 0b00100, 0b00000, 0b00000,
        ],
        ' ' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000,
        ],
        '_' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111,
        ],
        '-' => [
            0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000, 0b00000,
        ],
        _ => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000,
        ],
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct UiVertex {
    pos: [f32; 2],
    color: [f32; 4],
}

fn build_text_quads(
    lines: &[String],
    start_px: (f32, f32),
    surface_px: (f32, f32),
    out: &mut Vec<UiVertex>,
    color: [f32; 4],
    cell: f32,
) {
    let (sx, sy) = start_px;
    let (sw, sh) = surface_px;
    let glyph_w = 5.0 * cell;
    let glyph_h = 7.0 * cell;
    let line_gap = cell * 2.0;
    for (li, line) in lines.iter().enumerate() {
        let y_line = sy + li as f32 * (glyph_h + line_gap);
        let mut x_cursor = sx;
        for ch in line.chars() {
            let rows = glyph5x7_rows(ch);
            for (ry, row) in rows.iter().enumerate() {
                for cx in 0..5 {
                    if (row >> (4 - cx)) & 1 == 1 {
                        let px0 = x_cursor + cx as f32 * cell;
                        let py0 = y_line + ry as f32 * cell;
                        let px1 = px0 + cell;
                        let py1 = py0 + cell;
                        let x0 = -1.0 + px0 * 2.0 / sw;
                        let y0 = 1.0 - py0 * 2.0 / sh;
                        let x1 = -1.0 + px1 * 2.0 / sw;
                        let y1 = 1.0 - py1 * 2.0 / sh;
                        out.extend_from_slice(&[
                            UiVertex {
                                pos: [x0, y0],
                                color,
                            },
                            UiVertex {
                                pos: [x1, y0],
                                color,
                            },
                            UiVertex {
                                pos: [x1, y1],
                                color,
                            },
                            UiVertex {
                                pos: [x0, y0],
                                color,
                            },
                            UiVertex {
                                pos: [x1, y1],
                                color,
                            },
                            UiVertex {
                                pos: [x0, y1],
                                color,
                            },
                        ]);
                    }
                }
            }
            x_cursor += glyph_w + cell; // 1 cell spacing
        }
    }
}

#[derive(Clone)]
struct LibAnim {
    name: String,
    path: std::path::PathBuf,
}

#[derive(Clone)]
struct LibModel {
    name: String,
    path: std::path::PathBuf,
}

fn scan_anim_library() -> Vec<LibAnim> {
    let mut out: Vec<LibAnim> = Vec::new();
    let cwd = std::env::current_dir().unwrap_or_default();
    let repo_root = cwd; // assume running from repo root
    let mut candidates: Vec<std::path::PathBuf> = vec![repo_root.join("assets/anims")];
    if let Some(v) = std::env::var_os("FBX_LIB_DIR")
        && !v.is_empty()
    {
        candidates.push(std::path::PathBuf::from(v));
    }
    for dir in candidates.into_iter().filter(|p| p.exists()) {
        collect_anim_files(&dir, 0, 4, &mut out);
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn scan_model_library() -> Vec<LibModel> {
    let mut out: Vec<LibModel> = Vec::new();
    let cwd = std::env::current_dir().unwrap_or_default();
    let repo_root = cwd;
    let candidates = [repo_root.join("assets/models")];
    for dir in candidates.iter().filter(|p| p.exists()) {
        collect_model_files(dir, 0, 4, &mut out);
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn collect_model_files(
    dir: &std::path::Path,
    depth: usize,
    max_depth: usize,
    out: &mut Vec<LibModel>,
) {
    if depth > max_depth {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for ent in rd.flatten() {
        let p = ent.path();
        if p.is_dir() {
            collect_model_files(&p, depth + 1, max_depth, out);
            continue;
        }
        if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
            let ext_l = ext.to_ascii_lowercase();
            if ext_l == "gltf" || ext_l == "glb" {
                let name = p
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                out.push(LibModel { name, path: p });
            }
        }
    }
}

fn collect_anim_files(
    dir: &std::path::Path,
    depth: usize,
    max_depth: usize,
    out: &mut Vec<LibAnim>,
) {
    if depth > max_depth {
        return;
    }
    let rd = match std::fs::read_dir(dir) {
        Ok(d) => d,
        Err(_) => return,
    };
    for ent in rd.flatten() {
        let p = ent.path();
        if p.is_dir() {
            collect_anim_files(&p, depth + 1, max_depth, out);
            continue;
        }
        if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
            let ext_l = ext.to_ascii_lowercase();
            if ext_l == "fbx" || ext_l == "gltf" || ext_l == "glb" {
                let name = p
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                out.push(LibAnim { name, path: p });
            }
        }
    }
}

fn try_convert_fbx_to_gltf(src: &std::path::Path) -> Option<std::path::PathBuf> {
    let out_dir = std::path::Path::new("assets/anims/converted");
    let _ = std::fs::create_dir_all(out_dir);
    let stem = src.file_stem()?.to_str()?;
    let out_path = out_dir.join(format!("{}.glb", stem));
    if which::which("fbx2gltf").is_ok() {
        let status = std::process::Command::new("fbx2gltf")
            .arg("-b")
            .arg("-o")
            .arg(out_dir)
            .arg(src)
            .status()
            .ok()?;
        if status.success() && out_path.exists() {
            return Some(out_path);
        }
    }
    if which::which("assimp").is_ok() {
        let status = std::process::Command::new("assimp")
            .arg("export")
            .arg(src)
            .arg(&out_path)
            .status()
            .ok()?;
        if status.success() && out_path.exists() {
            return Some(out_path);
        }
    }
    None
}

// Animation CPU sampling helpers
struct AnimData {
    parent: Vec<Option<usize>>,
    base_t: Vec<Vec3>,
    base_r: Vec<glam::Quat>,
    base_s: Vec<Vec3>,
    joints_nodes: Vec<usize>,
    inverse_bind: Vec<Mat4>,
    node_names: Vec<String>,
    corr_mask: Option<Vec<bool>>, // optional head/neck correction mask
    corr_quat: glam::Quat,        // quaternion applied to masked nodes
    // Ordered clips parallel to displayed anims
    clips: Vec<ra_assets::AnimClip>,
}

impl AnimData {
    fn norm_bone_name(s: &str) -> String {
        let mut out = s.to_lowercase();
        for pref in [
            "def-",
            "rig|",
            "rig/",
            "rig:",
            "armature|",
            "armature/",
            "armature:",
            "skeleton|",
            "skeleton/",
            "skeleton:",
        ] {
            if out.starts_with(pref) {
                out = out.trim_start_matches(pref).to_string();
            }
            out = out.replace(pref, "");
        }
        out = out.replace([' ', '_', '-', '.', '|'], "");
        out.replace("hips", "pelvis")
            .replace("forearm", "lowerarm")
            .replace("shoulder", "clavicle")
            .replace("shin", "calf")
    }

    fn from_skinned_with_options(
        sk: &SkinnedMeshCPU,
        ordered_names: &[String],
        head_pitch_deg: f32,
    ) -> Self {
        let mut clips = Vec::new();
        for name in ordered_names {
            if let Some(c) = sk.animations.get(name) {
                clips.push(c.clone());
            } else {
                clips.push(ra_assets::AnimClip {
                    name: name.clone(),
                    duration: 0.0,
                    t_tracks: Default::default(),
                    r_tracks: Default::default(),
                    s_tracks: Default::default(),
                });
            }
        }
        let corr_q = glam::Quat::from_rotation_x(head_pitch_deg.to_radians());
        let corr_mask = if head_pitch_deg.abs() > 0.001 {
            let mut mask = vec![false; sk.parent.len()];
            for (i, n) in sk.node_names.iter().enumerate() {
                let nn = Self::norm_bone_name(n);
                let is_head = nn == "head";
                let is_neck = nn.starts_with("neck");
                let is_upper_spine = nn.starts_with("spine")
                    && (nn.ends_with("3")
                        || nn.ends_with("03")
                        || nn.ends_with("2")
                        || nn.ends_with("02")
                        || nn.contains("upper"));
                if is_head || is_neck || is_upper_spine {
                    mask[i] = true;
                }
            }
            Some(mask)
        } else {
            None
        };
        Self {
            parent: sk.parent.clone(),
            base_t: sk.base_t.clone(),
            base_r: sk.base_r.clone(),
            base_s: sk.base_s.clone(),
            joints_nodes: sk.joints_nodes.clone(),
            inverse_bind: sk.inverse_bind.clone(),
            node_names: sk.node_names.clone(),
            corr_mask,
            corr_quat: corr_q,
            clips,
        }
    }
    fn from_skinned(sk: &SkinnedMeshCPU, ordered_names: &[String]) -> Self {
        Self::from_skinned_with_options(sk, ordered_names, 0.0)
    }

    fn sample_palette(&self, clip_idx: usize, t: f32) -> Vec<[[f32; 4]; 4]> {
        let n = self.parent.len();
        let mut lt = self.base_t.clone();
        let mut lr = self.base_r.clone();
        let mut ls = self.base_s.clone();
        if let Some(clip) = self.clips.get(clip_idx) {
            let tt = &clip.t_tracks;
            let rt = &clip.r_tracks;
            let st = &clip.s_tracks;
            for (idx, tr) in tt {
                lt[*idx] = sample_vec3(tr, t);
            }
            for (idx, rr) in rt {
                lr[*idx] = sample_quat(rr, t);
            }
            for (idx, sr) in st {
                ls[*idx] = sample_vec3(sr, t);
            }
        }
        // Apply optional correction to head/neck rotations
        if let Some(mask) = &self.corr_mask {
            for (i, m) in mask.iter().enumerate() {
                if *m {
                    // Apply in local space after clip rotation
                    lr[i] = lr[i] * self.corr_quat;
                }
            }
        }
        // globals
        let mut g = vec![Mat4::IDENTITY; n];
        fn ensure(
            i: usize,
            parent: &[Option<usize>],
            lt: &[Vec3],
            lr: &[glam::Quat],
            ls: &[Vec3],
            g: &mut [Mat4],
        ) {
            if g[i] != Mat4::IDENTITY {
                return;
            }
            if let Some(p) = parent[i] {
                if g[p] == Mat4::IDENTITY {
                    ensure(p, parent, lt, lr, ls, g);
                }
                g[i] = g[p] * Mat4::from_scale_rotation_translation(ls[i], lr[i], lt[i]);
            } else {
                g[i] = Mat4::from_scale_rotation_translation(ls[i], lr[i], lt[i]);
            }
        }
        for i in 0..n {
            ensure(i, &self.parent, &lt, &lr, &ls, &mut g);
        }
        let mut out = Vec::with_capacity(self.joints_nodes.len());
        for (j, &node) in self.joints_nodes.iter().enumerate() {
            out.push((g[node] * self.inverse_bind[j]).to_cols_array_2d());
        }
        out
    }
}

fn sample_vec3(track: &ra_assets::TrackVec3, t: f32) -> Vec3 {
    let times = &track.times;
    let vals = &track.values;
    if times.is_empty() {
        return vals.first().copied().unwrap_or(Vec3::ZERO);
    }
    let dur = *times.last().unwrap_or(&0.0);
    let tt = if dur > 0.0 { t % dur } else { 0.0 };
    // find segment
    let mut i = 0;
    while i + 1 < times.len() && !(times[i] <= tt && tt <= times[i + 1]) {
        i += 1;
    }
    if i + 1 >= times.len() {
        return *vals.last().unwrap();
    }
    let t0 = times[i];
    let t1 = times[i + 1];
    let a = vals[i];
    let b = vals[i + 1];
    let w = if t1 > t0 { (tt - t0) / (t1 - t0) } else { 0.0 };
    a.lerp(b, w)
}

fn sample_quat(track: &ra_assets::TrackQuat, t: f32) -> glam::Quat {
    let times = &track.times;
    let vals = &track.values;
    if times.is_empty() {
        return vals.first().copied().unwrap_or(glam::Quat::IDENTITY);
    }
    let dur = *times.last().unwrap_or(&0.0);
    let tt = if dur > 0.0 { t % dur } else { 0.0 };
    let mut i = 0;
    while i + 1 < times.len() && !(times[i] <= tt && tt <= times[i + 1]) {
        i += 1;
    }
    if i + 1 >= times.len() {
        return *vals.last().unwrap();
    }
    let t0 = times[i];
    let t1 = times[i + 1];
    let a = vals[i];
    let b = vals[i + 1];
    let w = if t1 > t0 { (tt - t0) / (t1 - t0) } else { 0.0 };
    // Use shortest-arc slerp to avoid sudden flips at antipodal quats
    let mut bb = b;
    if a.dot(b) < 0.0 {
        bb = -b;
    }
    a.slerp(bb, w).normalize()
}

fn default_head_pitch_for(
    sk: &SkinnedMeshCPU,
    model_path: Option<&std::path::Path>,
    cli_pitch: f32,
) -> f32 {
    if cli_pitch.abs() > 0.001 {
        return cli_pitch;
    }
    if let Some(p) = model_path {
        if let Some(s) = p.to_str() {
            let sl = s.to_ascii_lowercase();
            if sl.contains("/ubc/")
                || sl.contains("superhero_male")
                || sl.contains("superhero_female")
            {
                return 45.0;
            }
        }
    }
    for n in &sk.node_names {
        let nl = n.to_ascii_lowercase();
        if nl.contains("superhero_male")
            || nl.contains("superhero_female")
            || nl == "head"
            || nl == "neck"
        {
            return 45.0;
        }
    }
    0.0
}

#[derive(Parser, Debug)]
#[command(name = "model-viewer")]
#[command(about = "Minimal wgpu model viewer (GLTF/GLB, baseColor, skin bind pose)")]
struct Cli {
    /// Optional path to a .gltf or .glb file; otherwise use drag-and-drop.
    path: Option<PathBuf>,

    /// Start in wireframe if supported
    #[arg(long)]
    wireframe: bool,

    /// Save a one-frame PNG snapshot and exit.
    #[arg(long)]
    snapshot: Option<PathBuf>,

    /// UI scale for text (1.0 = default). Lower to see more lines.
    #[arg(long, default_value_t = 1.0)]
    ui_scale: f32,

    /// Optional path to an animation library (GLTF/GLB/FBX) to merge into the loaded model.
    /// When provided with `path`, the clips are merged on load and the animation list is refreshed.
    #[arg(long)]
    anim_lib: Option<PathBuf>,

    /// Pitch correction in degrees to apply to head/neck bones (positive pitches up).
    /// Useful for rigs whose head rests pitched down in all clips.
    #[arg(long, default_value_t = 0.0)]
    head_pitch_deg: f32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct VSkinned {
    pos: [f32; 3],
    nrm: [f32; 3],
    uv: [f32; 2],
    joints: [u16; 4],
    weights: [f32; 4],
}

impl VSkinned {
    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<VSkinned>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                shader_location: 0,
                offset: 0,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                shader_location: 1,
                offset: 12,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                shader_location: 2,
                offset: 24,
                format: wgpu::VertexFormat::Float32x2,
            },
            wgpu::VertexAttribute {
                shader_location: 3,
                offset: 32,
                format: wgpu::VertexFormat::Uint16x4,
            },
            wgpu::VertexAttribute {
                shader_location: 4,
                offset: 40,
                format: wgpu::VertexFormat::Float32x4,
            },
        ],
    };
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct VBasic {
    pos: [f32; 3],
    nrm: [f32; 3],
}

impl VBasic {
    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<VBasic>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                shader_location: 0,
                offset: 0,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                shader_location: 1,
                offset: 12,
                format: wgpu::VertexFormat::Float32x3,
            },
        ],
    };
}

fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();
    pollster::block_on(run(cli))
}

#[allow(deprecated)]
async fn run(cli: Cli) -> Result<()> {
    // Window + surface
    let event_loop = EventLoop::new()?;
    let window = event_loop.create_window(
        WindowAttributes::default()
            .with_title("Model Viewer")
            .with_inner_size(PhysicalSize::new(1920, 1080)),
    )?;
    let instance = wgpu::Instance::default();
    let raw_display = window.display_handle()?.as_raw();
    let raw_window = window.window_handle()?.as_raw();
    let surface = unsafe {
        instance.create_surface_unsafe(SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: raw_display,
            raw_window_handle: raw_window,
        })
    }?;

    // Adapter/device
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
        })
        .await
        .expect("adapter");
    let needed_features = if cli.wireframe {
        wgpu::Features::POLYGON_MODE_LINE
    } else {
        wgpu::Features::empty()
    };
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("viewer-device"),
            required_features: needed_features,
            required_limits: wgpu::Limits::downlevel_defaults(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
        })
        .await?;

    // Surface config
    let size = window.inner_size();
    let caps = surface.get_capabilities(&adapter);
    let format = caps
        .formats
        .iter()
        .copied()
        .find(|f| f.is_srgb())
        .unwrap_or(caps.formats[0]);
    let present_mode = caps
        .present_modes
        .iter()
        .copied()
        .find(|m| *m == wgpu::PresentMode::Mailbox)
        .unwrap_or(wgpu::PresentMode::Fifo);
    let alpha_mode = caps.alpha_modes[0];
    let max_dim = device.limits().max_texture_dimension_2d.max(1);
    let (mut width, mut height) = scale_to_max((size.width, size.height), max_dim);
    let mut config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width,
        height,
        present_mode,
        alpha_mode,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &config);

    // Globals
    let globals = Globals {
        view_proj: Mat4::IDENTITY.to_cols_array_2d(),
    };
    let globals_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("globals"),
        contents: bytemuck::bytes_of(&globals),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });
    let globals_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("globals-bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("globals-bg"),
        layout: &globals_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: globals_buf.as_entire_binding(),
        }],
    });

    let skin_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("skin-bgl"),
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
    });
    // Material (base color) - created on demand per loaded model
    let mat_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mat-bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
        ],
    });
    // Model GPU state (populated on load)
    enum ModelGpu {
        Skinned {
            vb: wgpu::Buffer,
            ib: wgpu::Buffer,
            index_count: u32,
            mats: Vec<(wgpu::BindGroup, u32, u32)>, // (bind group, start, count)
            skin_bg: wgpu::BindGroup,
            skin_buf: wgpu::Buffer,
            center: Vec3,
            diag: f32,
            anims: Vec<String>,
            anim: Box<AnimData>,
            active_index: usize,
            time: f32,
            base: Box<SkinnedMeshCPU>,
        },
        Basic {
            vb: wgpu::Buffer,
            ib: wgpu::Buffer,
            index_count: u32,
            center: Vec3,
            diag: f32,
        },
    }
    let mut model_gpu: Option<ModelGpu> = None;

    // Animation library (scan assets/anims/* and optional env var FBX_LIB_DIR)
    let lib_anims: Vec<LibAnim> = scan_anim_library();
    // Model library (scan assets/models/*)
    let lib_models: Vec<LibModel> = scan_model_library();

    // Pipeline
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("skinned-shader"),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
            "shader_skinned.wgsl"
        ))),
    });
    let basic_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("basic-shader"),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
            "shader_basic.wgsl"
        ))),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("pl"),
        bind_group_layouts: &[&globals_bgl, &mat_bgl, &skin_bgl],
        push_constant_ranges: &[],
    });
    let depth_format = wgpu::TextureFormat::Depth32Float;
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("pipe"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main_skinned"),
            buffers: &[VSkinned::LAYOUT],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            polygon_mode: if cli.wireframe {
                wgpu::PolygonMode::Line
            } else {
                wgpu::PolygonMode::Fill
            },
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
    });

    // Basic (unskinned) pipeline: only globals bind group
    let basic_pl_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("pl-basic"),
        bind_group_layouts: &[&globals_bgl],
        push_constant_ranges: &[],
    });
    let basic_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("pipe-basic"),
        layout: Some(&basic_pl_layout),
        vertex: wgpu::VertexState {
            module: &basic_shader,
            entry_point: Some("vs_main"),
            buffers: &[VBasic::LAYOUT],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &basic_shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            polygon_mode: if cli.wireframe {
                wgpu::PolygonMode::Line
            } else {
                wgpu::PolygonMode::Fill
            },
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
    });

    let mut depth_view = create_depth(&device, width, height, depth_format);

    // Camera state
    let mut center = Vec3::ZERO;
    let mut diag = 1.0f32;
    // Orbit state
    let mut autorotate = true;
    let mut yaw: f32 = 0.0; // radians
    let mut pitch: f32 = 0.35; // radians, clamped
    let mut radius: f32 = 3.0;
    let mut rmb_down = false;
    let mut last_cursor: Option<(f32, f32)> = None;
    let mut mouse_pos_px: (f32, f32) = (0.0, 0.0);

    // Helper to upload a model: called on CLI path (if provided) and Drag&Drop
    let load_model = |path: &PathBuf,
                      device: &wgpu::Device,
                      queue: &wgpu::Queue,
                      mat_bgl: &wgpu::BindGroupLayout,
                      skin_bgl: &wgpu::BindGroupLayout|
     -> Result<ModelGpu> {
        let prepared = ra_assets::util::prepare_gltf_path(path)?;
        match load_gltf_skinned(&prepared) {
            Ok(skinned) => {
                info!(
                    "loaded (skinned): {} (verts={}, indices={}, joints={}, anims={})",
                    prepared.display(),
                    skinned.vertices.len(),
                    skinned.indices.len(),
                    skinned.joints_nodes.len(),
                    skinned.animations.len()
                );
                let vtx: Vec<VSkinned> = skinned
                    .vertices
                    .iter()
                    .map(|v| VSkinned {
                        pos: v.pos,
                        nrm: v.nrm,
                        uv: v.uv,
                        joints: v.joints,
                        weights: v.weights,
                    })
                    .collect();
                let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("vb"),
                    contents: bytemuck::cast_slice(&vtx),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("ib"),
                    contents: bytemuck::cast_slice(&skinned.indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
                let index_count = skinned.indices.len() as u32;
                let palette = compute_bind_pose_palette(&skinned);
                let skin_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("skin-palette"),
                    contents: bytemuck::cast_slice(&palette),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                });
                let skin_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("skin-bg"),
                    layout: skin_bgl,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: skin_buf.as_entire_binding(),
                    }],
                });
                // Create a reusable 1x1 white texture for submeshes missing a baseColor
                let white_tex = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("white"),
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
                        texture: &white_tex,
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
                let white_view = white_tex.create_view(&wgpu::TextureViewDescriptor::default());
                let linear_samp = device.create_sampler(&wgpu::SamplerDescriptor {
                    label: Some("samp"),
                    mag_filter: wgpu::FilterMode::Linear,
                    min_filter: wgpu::FilterMode::Linear,
                    mipmap_filter: wgpu::FilterMode::Nearest,
                    ..Default::default()
                });
                // Build per-submesh material bind groups
                let mut mats: Vec<(wgpu::BindGroup, u32, u32)> = Vec::new();
                if skinned.submeshes.is_empty() {
                    // Fallback: draw the whole mesh white
                    let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("mat-white"),
                        layout: mat_bgl,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::Sampler(&linear_samp),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::TextureView(&white_view),
                            },
                        ],
                    });
                    mats.push((bg, 0, skinned.indices.len() as u32));
                } else {
                    for sm in &skinned.submeshes {
                        let (view, samp) = if let Some(tex) = &sm.base_color_texture {
                            let size = wgpu::Extent3d {
                                width: tex.width,
                                height: tex.height,
                                depth_or_array_layers: 1,
                            };
                            let t = device.create_texture(&wgpu::TextureDescriptor {
                                label: Some("albedo"),
                                size,
                                mip_level_count: 1,
                                sample_count: 1,
                                dimension: wgpu::TextureDimension::D2,
                                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                                usage: wgpu::TextureUsages::TEXTURE_BINDING
                                    | wgpu::TextureUsages::COPY_DST,
                                view_formats: &[],
                            });
                            queue.write_texture(
                                wgpu::TexelCopyTextureInfo {
                                    texture: &t,
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
                                size,
                            );
                            (
                                t.create_view(&wgpu::TextureViewDescriptor::default()),
                                linear_samp.clone(),
                            )
                        } else {
                            (white_view.clone(), linear_samp.clone())
                        };
                        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("mat-submesh"),
                            layout: mat_bgl,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::Sampler(&samp),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: wgpu::BindingResource::TextureView(&view),
                                },
                            ],
                        });
                        mats.push((bg, sm.start, sm.count));
                    }
                }
                let (min_b, max_b) = compute_bounds(&skinned);
                let center = 0.5 * (min_b + max_b);
                let diag = (max_b - min_b).length().max(1.0);
                // Collect animation names
                let mut names: Vec<String> = skinned.animations.keys().cloned().collect();
                names.sort();
                // Move skinned into model state as base
                let base = Box::new(skinned);
                let auto_pitch = default_head_pitch_for(&base, Some(&prepared), cli.head_pitch_deg);
                if auto_pitch.abs() > 0.001 {
                    log::info!("viewer: head pitch correction {} deg", auto_pitch);
                }
                let anim = Box::new(AnimData::from_skinned_with_options(
                    &base, &names, auto_pitch,
                ));
                Ok(ModelGpu::Skinned {
                    vb,
                    ib,
                    index_count,
                    mats,
                    skin_bg,
                    skin_buf,
                    center,
                    diag,
                    anims: names,
                    anim,
                    active_index: 0,
                    time: 0.0,
                    base,
                })
            }
            Err(e) => {
                log::warn!("skinned load failed, trying unskinned: {}", e);
                let cpu: CpuMesh = load_gltf_mesh(&prepared)?;
                info!(
                    "loaded (basic): {} (verts={}, indices={})",
                    prepared.display(),
                    cpu.vertices.len(),
                    cpu.indices.len()
                );
                let verts: Vec<VBasic> = cpu
                    .vertices
                    .iter()
                    .map(|v| VBasic {
                        pos: v.pos,
                        nrm: v.nrm,
                    })
                    .collect();
                let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("vb-basic"),
                    contents: bytemuck::cast_slice(&verts),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("ib-basic"),
                    contents: bytemuck::cast_slice(&cpu.indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
                let index_count = cpu.indices.len() as u32;
                let (min_b, max_b) = compute_bounds_basic(&cpu);
                let center = 0.5 * (min_b + max_b);
                let diag = (max_b - min_b).length().max(1.0);
                Ok(ModelGpu::Basic {
                    vb,
                    ib,
                    index_count,
                    center,
                    diag,
                })
            }
        }
    };

    // Load from CLI if provided
    if let Some(p) = cli.path.as_ref() {
        if let Ok(mut gpu) = load_model(p, &device, &queue, &mat_bgl, &skin_bgl) {
            match &gpu {
                ModelGpu::Skinned {
                    center: c,
                    diag: d,
                    anims,
                    ..
                } => {
                    center = *c;
                    diag = *d;
                    radius = *d * 1.0;
                    yaw = 0.0;
                    pitch = 0.35;
                    let title = format!(
                        "Model Viewer — {} | anims: {}",
                        p.display(),
                        if anims.is_empty() {
                            "(none)".to_string()
                        } else {
                            anims.join(", ")
                        }
                    );
                    window.set_title(&title);
                }
                ModelGpu::Basic {
                    center: c, diag: d, ..
                } => {
                    center = *c;
                    diag = *d;
                    radius = *d * 1.0;
                    yaw = 0.0;
                    pitch = 0.35;
                    let title = format!("Model Viewer — {} | anims: (none)", p.display());
                    window.set_title(&title);
                }
            }
            // If an animation library is provided, and the model is skinned, merge now.
            if let Some(lib_path) = cli.anim_lib.as_ref() {
                if let ModelGpu::Skinned {
                    base,
                    anim,
                    anims,
                    time,
                    active_index,
                    ..
                } = &mut gpu
                {
                    let ext = lib_path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    let mut merged_ok = false;
                    if ext == "gltf" || ext == "glb" {
                        if let Ok(n) = merge_gltf_animations(base.as_mut(), lib_path) {
                            merged_ok = n > 0;
                            log::info!(
                                "viewer: merged {} GLTF animations from {}",
                                n,
                                lib_path.display()
                            );
                        }
                    } else if ext == "fbx" {
                        if merge_fbx_animations(base.as_mut(), lib_path).is_ok() {
                            merged_ok = true;
                            log::info!("viewer: merged FBX animations from {}", lib_path.display());
                        } else if let Some(conv) = try_convert_fbx_to_gltf(lib_path) {
                            if let Ok(n) = merge_gltf_animations(base.as_mut(), &conv) {
                                merged_ok = n > 0;
                                log::info!(
                                    "viewer: merged {} animations from converted {}",
                                    n,
                                    conv.display()
                                );
                            }
                        }
                    }
                    if merged_ok {
                        let mut names: Vec<String> = base.animations.keys().cloned().collect();
                        names.sort();
                        **anim =
                            AnimData::from_skinned_with_options(&base, &names, cli.head_pitch_deg);
                        *anims = names;
                        *time = 0.0;
                        *active_index = 0;
                    }
                }
            }
            model_gpu = Some(gpu);
        } else {
            log::error!("failed to load {}", p.display());
        }
    } else {
        info!("Drag-and-drop a .gltf or .glb into this window");
    }

    let mut snapshot_path = cli.snapshot.clone();
    let mut snapshot_done = false;
    Ok(event_loop.run(move |event, elwt| match event {
        Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => elwt.exit(),
        Event::WindowEvent { event: WindowEvent::Resized(new_size), .. } => {
            (width, height) = scale_to_max((new_size.width, new_size.height), max_dim);
            config.width = width.max(1);
            config.height = height.max(1);
            surface.configure(&device, &config);
            depth_view = create_depth(&device, width, height, depth_format);
        }
        Event::AboutToWait => {
            // Always animate; request a redraw every frame
            window.request_redraw();
        }
        Event::WindowEvent { event: WindowEvent::RedrawRequested, .. } => {
            // Update animation palette (if skinned)
            if let Some(ModelGpu::Skinned { skin_buf, anim, active_index, time, .. }) = model_gpu.as_mut() {
                let now = std::time::Instant::now();
                static mut LAST: Option<std::time::Instant> = None;
                let dt = unsafe { match LAST { Some(prev) => { let d = now.duration_since(prev).as_secs_f32(); LAST = Some(now); d }, None => { LAST = Some(now); 0.0 } } };
                *time += dt;
                let palette = anim.sample_palette(*active_index, *time);
                queue.write_buffer(skin_buf, 0, bytemuck::cast_slice(&palette));
            }
            // Orbit camera (mouse + optional autorotate)
            if autorotate { yaw += 0.6 / 60.0; }
            let cp = pitch.clamp(-1.2, 1.2);
            let r = radius.max(0.05);
            let eye = center + Vec3::new(r * cp.cos() * yaw.cos(), r * cp.sin(), r * cp.cos() * yaw.sin());
            let view = Mat4::look_at_rh(eye, center, Vec3::Y);
            let proj = Mat4::perspective_rh_gl(60f32.to_radians(), width as f32 / height as f32, 0.05, 100.0 * diag);
            let vp = (proj * view).to_cols_array_2d();
            queue.write_buffer(&globals_buf, 0, bytemuck::bytes_of(&Globals { view_proj: vp }));

            let frame = match surface.get_current_texture() { Ok(f) => f, Err(_) => { surface.configure(&device, &config); surface.get_current_texture().expect("frame") } };
            let view_tex = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("enc") });
            // Optional offscreen target for snapshot
            let mut snap_tex: Option<wgpu::Texture> = None;
            let mut snap_view: Option<wgpu::TextureView> = None;
            if snapshot_path.is_some() && !snapshot_done {
                let t = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("snapshot-rt"),
                    size: wgpu::Extent3d { width: config.width, height: config.height, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                });
                snap_view = Some(t.create_view(&wgpu::TextureViewDescriptor::default()));
                snap_tex = Some(t);
            }
            {
                let mut rpass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("rpass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: if let Some(ref v) = snap_view { v } else { &view_tex }, resolve_target: None, depth_slice: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.03, g: 0.03, b: 0.05, a: 1.0 }), store: wgpu::StoreOp::Store } })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &depth_view, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });
                if let Some(ref gpu) = model_gpu {
                    match gpu {
                        ModelGpu::Skinned { vb, ib, mats, skin_bg, .. } => {
                            rpass.set_pipeline(&pipeline);
                            rpass.set_bind_group(0, &globals_bg, &[]);
                            rpass.set_bind_group(2, skin_bg, &[]);
                            rpass.set_vertex_buffer(0, vb.slice(..));
                            rpass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint16);
                            for (bg, start, count) in mats {
                                rpass.set_bind_group(1, bg, &[]);
                                rpass.draw_indexed(*start..(*start + *count), 0, 0..1);
                            }
                        }
                        ModelGpu::Basic { vb, ib, index_count, .. } => {
                            rpass.set_pipeline(&basic_pipeline);
                            rpass.set_bind_group(0, &globals_bg, &[]);
                            rpass.set_vertex_buffer(0, vb.slice(..));
                            rpass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint16);
                            rpass.draw_indexed(0..*index_count, 0, 0..1);
                        }
                    }
                }
            }
            // Second pass: simple UI checkbox for autorotate in top-left
            {
                let mut rpass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("ui-pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: if let Some(ref v) = snap_view { v } else { &view_tex },
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });
                // Build a tiny checkbox from 2 triangles (outline + fill when on)
                let s: f32 = 20.0; // px
                let m: f32 = 16.0; // margin px
                let x0 = -1.0 + m * 2.0 / (width as f32);
                let y0 = 1.0 - m * 2.0 / (height as f32);
                let x1 = -1.0 + (m + s) * 2.0 / (width as f32);
                let y1 = 1.0 - (m + s) * 2.0 / (height as f32);
                let mut verts: Vec<UiVertex> = vec![
                    UiVertex { pos: [x0, y0], color: [0.15, 0.15, 0.18, 1.0] },
                    UiVertex { pos: [x1, y0], color: [0.15, 0.15, 0.18, 1.0] },
                    UiVertex { pos: [x1, y1], color: [0.15, 0.15, 0.18, 1.0] },
                    UiVertex { pos: [x0, y0], color: [0.15, 0.15, 0.18, 1.0] },
                    UiVertex { pos: [x1, y1], color: [0.15, 0.15, 0.18, 1.0] },
                    UiVertex { pos: [x0, y1], color: [0.15, 0.15, 0.18, 1.0] },
                ];
                if autorotate {
                    let pad = 4.0;
                    let ix0 = -1.0 + (m + pad) * 2.0 / (width as f32);
                    let iy0 = 1.0 - (m + pad) * 2.0 / (height as f32);
                    let ix1 = -1.0 + (m + s - pad) * 2.0 / (width as f32);
                    let iy1 = 1.0 - (m + s - pad) * 2.0 / (height as f32);
                    let c = [0.2, 0.85, 0.3, 1.0];
                    verts.extend_from_slice(&[
                        UiVertex { pos: [ix0, iy0], color: c }, UiVertex { pos: [ix1, iy0], color: c }, UiVertex { pos: [ix1, iy1], color: c },
                        UiVertex { pos: [ix0, iy0], color: c }, UiVertex { pos: [ix1, iy1], color: c }, UiVertex { pos: [ix0, iy1], color: c },
                    ]);
                }
                // Minimal UI pipeline inline (position+color in NDC)
                let ui_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("ui-shader"),
                    source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed("struct VSIn{ @location(0) pos: vec2<f32>, @location(1) color: vec4<f32>, }; struct VSOut{ @builtin(position) pos: vec4<f32>, @location(0) color: vec4<f32>, }; @vertex fn vs_main(v:VSIn)->VSOut{ var o:VSOut; o.pos=vec4<f32>(v.pos,0.0,1.0); o.color=v.color; return o; } @fragment fn fs_main(i:VSOut)->@location(0) vec4<f32>{ return i.color; }")),
                });
                let ui_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("ui-pl"), bind_group_layouts: &[], push_constant_ranges: &[] });
                let ui_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("ui-pipe"), layout: Some(&ui_pl),
                    vertex: wgpu::VertexState { module: &ui_shader, entry_point: Some("vs_main"), buffers: &[wgpu::VertexBufferLayout{ array_stride: (6*4) as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &[
                        wgpu::VertexAttribute{ shader_location:0, offset:0, format: wgpu::VertexFormat::Float32x2 },
                        wgpu::VertexAttribute{ shader_location:1, offset:8, format: wgpu::VertexFormat::Float32x4 },
                    ]}], compilation_options: Default::default() },
                    fragment: Some(wgpu::FragmentState { module: &ui_shader, entry_point: Some("fs_main"), targets: &[Some(wgpu::ColorTargetState{ format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                    cache: None,
                });
                let ui_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor{ label: Some("ui-vb"), contents: bytemuck::cast_slice(&verts), usage: wgpu::BufferUsages::VERTEX });
                rpass.set_pipeline(&ui_pipe);
                rpass.set_vertex_buffer(0, ui_vb.slice(..));
                rpass.draw(0..(verts.len() as u32), 0..1);

                // Text overlay: label next to checkbox + list animations beneath
                // Draw label next to the checkbox (always)
                let mut text_verts_label: Vec<UiVertex> = Vec::new();
                let label = vec!["AUTO ROTATE".to_string()];
                let label_start = (m + s + 8.0, m);
                let label_cell = 3.0 * cli.ui_scale.max(0.25);
                build_text_quads(&label, label_start, (width as f32, height as f32), &mut text_verts_label, [0.85, 0.85, 0.9, 1.0], label_cell);
                if !text_verts_label.is_empty() {
                    let tvb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("ui-label"), contents: bytemuck::cast_slice(&text_verts_label), usage: wgpu::BufferUsages::VERTEX });
                    rpass.set_pipeline(&ui_pipe);
                    rpass.set_vertex_buffer(0, tvb.slice(..));
                    rpass.draw(0..(text_verts_label.len() as u32), 0..1);
                }
                // Anim list text (if any)
                let mut text_verts: Vec<UiVertex> = Vec::new();
                let mut lines: Vec<String> = Vec::new();
                if let Some(ref gpu) = model_gpu
                    && let ModelGpu::Skinned { anims, .. } = gpu
                    && !anims.is_empty()
                {
                    lines.push("ANIMATIONS:".to_string());
                    for (i, name) in anims.iter().enumerate() {
                        lines.push(format!("{}: {}", i + 1, name.to_uppercase()));
                    }
                }
                if !lines.is_empty() {
                    let anim_cell: f32 = 6.0 * cli.ui_scale.max(0.25); // animation list text
                    let start_px = (m, m + s + 8.0);
                    build_text_quads(&lines, start_px, (width as f32, height as f32), &mut text_verts, [0.9, 0.9, 0.95, 1.0], anim_cell);
                    let tvb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("ui-text"), contents: bytemuck::cast_slice(&text_verts), usage: wgpu::BufferUsages::VERTEX });
                    rpass.set_pipeline(&ui_pipe);
                    rpass.set_vertex_buffer(0, tvb.slice(..));
                    rpass.draw(0..(text_verts.len() as u32), 0..1);
                }

                // Model library list
                if !lib_models.is_empty() {
                    let mut model_lines: Vec<String> = vec!["MODELS:".to_string()];
                    for (i, mentry) in lib_models.iter().enumerate() {
                        model_lines.push(format!("{}: {}", i + 1, mentry.name.to_uppercase()));
                    }
                    let anim_cell: f32 = 6.0 * cli.ui_scale.max(0.25);
                    let base_y = m + s + 8.0;
                    let anim_lines = if let Some(ModelGpu::Skinned{anims, ..}) = &model_gpu { anims.len() as f32 + 1.0 } else { 0.0 };
                    let y_offset = base_y + anim_lines * ((7.0*anim_cell) + (anim_cell*2.0)) + 10.0;
                    let mut model_text: Vec<UiVertex> = Vec::new();
                    build_text_quads(&model_lines, (m, y_offset), (width as f32, height as f32), &mut model_text, [0.8,0.9,0.95,1.0], anim_cell);
                    if !model_text.is_empty() {
                        let tvb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("ui-models"), contents: bytemuck::cast_slice(&model_text), usage: wgpu::BufferUsages::VERTEX });
                        rpass.set_pipeline(&ui_pipe);
                        rpass.set_vertex_buffer(0, tvb.slice(..));
                        rpass.draw(0..(model_text.len() as u32), 0..1);
                    }
                }

                // Library animations (if any)
                if !lib_anims.is_empty() {
                    let mut lib_lines: Vec<String> = vec!["LIBRARY:".to_string()];
                    for (i, a) in lib_anims.iter().enumerate() { lib_lines.push(format!("{}: {}", i + 1, a.name.to_uppercase())); }
                    let anim_cell: f32 = 6.0 * cli.ui_scale.max(0.25);
                    let base_y = m + s + 8.0;
                    let anim_lines = if let Some(ModelGpu::Skinned{anims, ..}) = &model_gpu { anims.len() as f32 + 1.0 } else { 0.0 };
                    let model_lines = if lib_models.is_empty() { 0.0 } else { lib_models.len() as f32 + 1.0 };
                    let y_offset = base_y + (anim_lines + model_lines) * ((7.0*anim_cell) + (anim_cell*2.0)) + 10.0;
                    let mut lib_text: Vec<UiVertex> = Vec::new();
                    build_text_quads(&lib_lines, (m, y_offset), (width as f32, height as f32), &mut lib_text, [0.8,0.85,0.95,1.0], anim_cell);
                    if !lib_text.is_empty() {
                        let tvb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("ui-lib"), contents: bytemuck::cast_slice(&lib_text), usage: wgpu::BufferUsages::VERTEX });
                        rpass.set_pipeline(&ui_pipe);
                        rpass.set_vertex_buffer(0, tvb.slice(..));
                        rpass.draw(0..(lib_text.len() as u32), 0..1);
                    }
                }
            }

            // Snapshot support: copy swapchain to buffer and write PNG once, then exit
            if !snapshot_done {
                if let Some(path) = snapshot_path.take() {
                    let bpp: u32 = 4;
                    let unpadded = config.width * bpp;
                    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as u32;
                    let padded = ((unpadded + align - 1) / align) * align;
                    let read_size = (padded * config.height) as u64;
                    let read_buf = device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("snapshot-read"),
                        size: read_size,
                        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                    let src = if let Some(ref tex) = snap_tex { tex.as_image_copy() } else { frame.texture.as_image_copy() };
                    enc.copy_texture_to_buffer(
                        src,
                        wgpu::TexelCopyBufferInfo {
                            buffer: &read_buf,
                            layout: wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(padded),
                                rows_per_image: Some(config.height),
                            },
                        },
                        wgpu::Extent3d { width: config.width, height: config.height, depth_or_array_layers: 1 },
                    );
                    queue.submit(Some(enc.finish()));
                    // Map and wait using a channel; submit an empty queue op to pump callbacks
                    let slice = read_buf.slice(..);
                    let (tx, rx) = std::sync::mpsc::channel();
                    slice.map_async(wgpu::MapMode::Read, move |v| { let _ = tx.send(v); });
                    queue.submit(std::iter::empty());
                    let _ = rx.recv();
                    let data = slice.get_mapped_range();
                    let mut out_rgba = vec![0u8; (config.width * config.height * 4) as usize];
                    for y in 0..config.height {
                        let src = &data[(y * padded) as usize..(y * padded + unpadded) as usize];
                        let dst = &mut out_rgba[(y * unpadded) as usize..(y * unpadded + unpadded) as usize];
                        if matches!(format, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb) {
                            for x in 0..config.width {
                                let i = (x * 4) as usize;
                                dst[i] = src[i + 2];
                                dst[i + 1] = src[i + 1];
                                dst[i + 2] = src[i + 0];
                                dst[i + 3] = src[i + 3];
                            }
                        } else {
                            dst.copy_from_slice(&src[..(unpadded as usize)]);
                        }
                    }
                    drop(data);
                    read_buf.unmap();
                    if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }
                    let file = std::fs::File::create(&path).expect("snapshot file");
                    let mut enc_png = png::Encoder::new(file, config.width, config.height);
                    enc_png.set_color(png::ColorType::Rgba);
                    enc_png.set_depth(png::BitDepth::Eight);
                    let mut writer = enc_png.write_header().unwrap();
                    writer.write_image_data(&out_rgba).unwrap();
                    snapshot_done = true;
                    frame.present();
                    elwt.exit();
                    return;
                }
            }
            queue.submit(Some(enc.finish()));
            frame.present();
        }
        Event::WindowEvent { event: WindowEvent::DroppedFile(path), .. } => {
            if let Ok(gpu) = load_model(&path, &device, &queue, &mat_bgl, &skin_bgl) {
                match &gpu {
                    ModelGpu::Skinned { center: c, diag: d, anims, .. } => {
                        center = *c; diag = *d; radius = *d * 1.0; yaw = 0.0; pitch = 0.35;
                        let title = format!("Model Viewer — {} | anims: {}", path.display(), if anims.is_empty() { "(none)".to_string() } else { anims.join(", ") });
                        window.set_title(&title);
                    }
                    ModelGpu::Basic { center: c, diag: d, .. } => {
                        center = *c; diag = *d; radius = *d * 1.0; yaw = 0.0; pitch = 0.35;
                        let title = format!("Model Viewer — {} | anims: (none)", path.display());
                        window.set_title(&title);
                    }
                }
                model_gpu = Some(gpu);
            } else {
                log::error!("failed to load {}", path.display());
            }
        }
        Event::WindowEvent { event: WindowEvent::MouseInput{ state, button: MouseButton::Right, .. }, .. } => {
            rmb_down = state == ElementState::Pressed;
            if !rmb_down { last_cursor = None; }
        }
        Event::WindowEvent { event: WindowEvent::CursorMoved { position, .. }, .. } => {
            mouse_pos_px = (position.x as f32, position.y as f32);
            if rmb_down {
                if let Some((lx, ly)) = last_cursor {
                    let dx = (position.x as f32 - lx) / (width as f32);
                    let dy = (position.y as f32 - ly) / (height as f32);
                    yaw -= dx * std::f32::consts::TAU; // 1 full turn across window width
                    // Flip vertical: moving mouse up should tilt camera up (decrease pitch)
                    pitch += dy * std::f32::consts::PI; // half-turn across height
                    pitch = pitch.clamp(-1.2, 1.2);
                }
                last_cursor = Some((position.x as f32, position.y as f32));
            }
        }
        Event::WindowEvent { event: WindowEvent::MouseWheel { delta, .. }, .. } => {
            let scroll = match delta { MouseScrollDelta::LineDelta(_, y) => y * 50.0, MouseScrollDelta::PixelDelta(p) => p.y as f32 };
            radius *= (1.0 - scroll * 0.001).max(0.1);
        }
        Event::WindowEvent { event: WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. }, .. } => {
            // Toggle autorotate if clicking checkbox or its label area
            let s: f32 = 20.0; let m: f32 = 16.0;
            let (mx, my) = mouse_pos_px;
            let in_box = mx >= m && mx <= m + s && my >= m && my <= m + s;
            // Label rect
            let cell = 3.0; let label = "AUTO ROTATE"; let label_w = label.len() as f32 * 6.0 * cell; let label_h = 7.0 * cell;
            let lx0 = m + s + 8.0; let ly0 = m; let lx1 = lx0 + label_w; let ly1 = ly0 + label_h;
            let in_label = mx >= lx0 && mx <= lx1 && my >= ly0 && my <= ly1;
            if in_box || in_label { autorotate = !autorotate; }
            // Animation buttons (skinned): click lines under header
            if let Some(gpu) = model_gpu.as_mut()
                && let ModelGpu::Skinned { anims, active_index, time, .. } = gpu
                && !anims.is_empty()
            {
                let list_x = m;
                let list_y = m + s + 8.0;
                // Match the text layout used for drawing (scaled by --ui-scale)
                let anim_cell: f32 = 6.0 * cli.ui_scale.max(0.25);
                let glyph_w = 5.0 * anim_cell;
                let glyph_h = 7.0 * anim_cell;
                // Header line occupies first row; buttons start at i=1
                for (i, name) in anims.iter().enumerate() {
                    let text = format!("{}: {}", i + 1, name.to_uppercase());
                    let tx0 = list_x;
                    let ty0 = list_y + (i as f32 + 1.0) * (glyph_h + anim_cell * 2.0); // +1 to skip header
                    let tw = text.len() as f32 * (glyph_w + anim_cell);
                    let th = glyph_h;
                    if mx >= tx0 && mx <= tx0 + tw && my >= ty0 && my <= ty0 + th {
                        *active_index = i;
                        *time = 0.0;
                    }
                }
            }
            // Model entries click handling (replace base model)
            if !lib_models.is_empty() {
                let s: f32 = 20.0; let m: f32 = 16.0;
                let anim_cell: f32 = 6.0 * cli.ui_scale.max(0.25);
                let base_y = m + s + 8.0;
                let anim_lines = if let Some(ModelGpu::Skinned{anims, ..}) = &model_gpu { anims.len() as f32 + 1.0 } else { 0.0 };
                let y_offset = base_y + anim_lines * ((7.0*anim_cell) + (anim_cell*2.0)) + 10.0;
                for (i, entry) in lib_models.clone().into_iter().enumerate() {
                    let text = format!("{}: {}", i + 1, entry.name.to_uppercase());
                    let glyph_w = 5.0 * anim_cell; let glyph_h = 7.0 * anim_cell;
                    let tx0 = m; let ty0 = y_offset + (i as f32 + 1.0) * (glyph_h + anim_cell * 2.0);
                    let tw = text.len() as f32 * (glyph_w + anim_cell); let th = glyph_h;
                    if mx >= tx0 && mx <= tx0 + tw && my >= ty0 && my <= ty0 + th {
                        if let Ok(gpu) = load_model(&entry.path, &device, &queue, &mat_bgl, &skin_bgl) {
                            model_gpu = Some(gpu);
                        }
                        break;
                    }
                }
            }

            // Library entries click handling (merge into loaded model)
            if let Some(gpu) = model_gpu.as_mut()
                && let ModelGpu::Skinned { anims, active_index, time, anim, base, .. } = gpu
                && !lib_anims.is_empty()
            {
                let mut replace_new: Option<ModelGpu> = None;
                let base_y = m + s + 8.0;
                let anim_cell: f32 = 6.0 * cli.ui_scale.max(0.25);
                let glyph_w = 5.0 * anim_cell; let glyph_h = 7.0 * anim_cell;
                let model_lines = (anims.len() as f32 + 1.0).max(0.0);
                let y_offset = base_y + model_lines * (glyph_h + anim_cell*2.0) + 10.0;
                for (i, a) in lib_anims.clone().into_iter().enumerate() {
                    let text = format!("{}: {}", i + 1, a.name.to_uppercase());
                    let tx0 = m; let ty0 = y_offset + (i as f32 + 1.0) * (glyph_h + anim_cell * 2.0);
                    let tw = text.len() as f32 * (glyph_w + anim_cell); let th = glyph_h;
                    if mx >= tx0 && mx <= tx0 + tw && my >= ty0 && my <= ty0 + th {
                        let ext = a.path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
                        if ext == "gltf" || ext == "glb" {
                            // Prefer merging GLTF animations into the current model
                            match merge_gltf_animations(base, &a.path) {
                                Ok(_n) => {
                                    let mut new_names: Vec<String> = base.animations.keys().cloned().collect(); new_names.sort();
                                    let auto_pitch = default_head_pitch_for(base, None, cli.head_pitch_deg);
                                    if auto_pitch.abs() > 0.001 { log::info!("viewer: head pitch correction {} deg", auto_pitch); }
                                    *anim = Box::new(AnimData::from_skinned_with_options(base, &new_names, auto_pitch));
                                    *anims = new_names; *time = 0.0; *active_index = 0;
                                    log::info!("viewer: merged GLTF animations from {}", a.path.display());
                                }
                                Err(e) => {
                                    log::warn!("viewer: merge GLTF animations failed ({}), replacing model", e);
                                    if let Ok(new_gpu) = load_model(&a.path, &device, &queue, &mat_bgl, &skin_bgl) { replace_new = Some(new_gpu); }
                                }
                            }
                        } else if ext == "fbx" {
                            // Merge FBX as animations into current model
                            if merge_fbx_animations(base, &a.path).is_err() && let Some(conv) = try_convert_fbx_to_gltf(&a.path) { let _ = merge_gltf_animations(base, &conv); }
                            // refresh AnimData and UI list
                            let mut new_names: Vec<String> = base.animations.keys().cloned().collect(); new_names.sort();
                            let auto_pitch = default_head_pitch_for(base, None, cli.head_pitch_deg);
                            if auto_pitch.abs() > 0.001 { log::info!("viewer: head pitch correction {} deg", auto_pitch); }
                            *anim = Box::new(AnimData::from_skinned_with_options(base, &new_names, auto_pitch));
                            *anims = new_names;
                            *time = 0.0; *active_index = anim.clips.len().saturating_sub(1);
                        }
                        break;
                    }
                }
                if let Some(new_gpu) = replace_new {
                    model_gpu = Some(new_gpu);
                    match model_gpu.as_ref().unwrap() {
                        ModelGpu::Skinned { center: c, diag: d, anims: new_anims, .. } => {
                            center = *c; diag = *d; radius = *d * 1.0; yaw = 0.0; pitch = 0.35;
                            let title = format!("Model Viewer — (library) | anims: {}", if new_anims.is_empty() { "(none)".to_string() } else { new_anims.join(", ") });
                            window.set_title(&title);
                        }
                        ModelGpu::Basic { center: c, diag: d, .. } => {
                            center = *c; diag = *d; radius = *d * 1.0; yaw = 0.0; pitch = 0.35;
                            window.set_title("Model Viewer — (library) | anims: (none)");
                        }
                    }
                }
            }
            // Library entries click handling when no model is loaded: load as base model
            if model_gpu.is_none() && !lib_anims.is_empty() {
                let base_y = m + s + 8.0;
                let anim_cell = 6.0; let glyph_w = 5.0 * anim_cell; let glyph_h = 7.0 * anim_cell;
                let model_lines = 0.0f32; // no animations yet
                let y_offset = base_y + model_lines * (glyph_h + anim_cell*2.0) + 10.0;
                for (i, a) in lib_anims.clone().into_iter().enumerate() {
                    let text = format!("{}: {}", i + 1, a.name.to_uppercase());
                    let tx0 = m; let ty0 = y_offset + (i as f32 + 1.0) * (glyph_h + anim_cell * 2.0);
                    let tw = text.len() as f32 * (glyph_w + anim_cell); let th = glyph_h;
                    if mx >= tx0 && mx <= tx0 + tw && my >= ty0 && my <= ty0 + th {
                        // Load this library model as the base
                        if let Ok(gpu) = load_model(&a.path, &device, &queue, &mat_bgl, &skin_bgl) {
                            match &gpu {
                                ModelGpu::Skinned { center: c, diag: d, anims, .. } => {
                                    center = *c; diag = *d; radius = *d * 1.0; yaw = 0.0; pitch = 0.35;
                                    let title = format!("Model Viewer — {} | anims: {}", a.path.display(), if anims.is_empty() { "(none)".to_string() } else { anims.join(", ") });
                                    window.set_title(&title);
                                }
                                ModelGpu::Basic { center: c, diag: d, .. } => {
                                    center = *c; diag = *d; radius = *d * 1.0; yaw = 0.0; pitch = 0.35;
                                    let title = format!("Model Viewer — {} | anims: (none)", a.path.display());
                                    window.set_title(&title);
                                }
                            }
                            model_gpu = Some(gpu);
                        }
                        break;
                    }
                }
            }
        }
        _ => {}
    })?)
}

fn compute_bind_pose_palette(model: &SkinnedMeshCPU) -> Vec<[[f32; 4]; 4]> {
    let n = model.parent.len();
    let mut globals = vec![Mat4::IDENTITY; n];
    // Compute global matrices via DFS
    fn compute(
        i: usize,
        parent: &[Option<usize>],
        t: &[Vec3],
        r: &[glam::Quat],
        s: &[Vec3],
        out: &mut [Mat4],
    ) {
        if out[i] != Mat4::IDENTITY {
            return;
        }
        if let Some(p) = parent[i] {
            if out[p] == Mat4::IDENTITY {
                compute(p, parent, t, r, s, out);
            }
            out[i] = out[p] * Mat4::from_scale_rotation_translation(s[i], r[i], t[i]);
        } else {
            out[i] = Mat4::from_scale_rotation_translation(s[i], r[i], t[i]);
        }
    }
    for i in 0..n {
        compute(
            i,
            &model.parent,
            &model.base_t,
            &model.base_r,
            &model.base_s,
            &mut globals,
        );
    }
    let mut palette: Vec<[[f32; 4]; 4]> = Vec::with_capacity(model.joints_nodes.len());
    for (j, &node_idx) in model.joints_nodes.iter().enumerate() {
        let m = globals[node_idx] * model.inverse_bind[j];
        palette.push(m.to_cols_array_2d());
    }
    palette
}

fn compute_bounds(model: &SkinnedMeshCPU) -> (Vec3, Vec3) {
    let mut min_b = Vec3::splat(f32::INFINITY);
    let mut max_b = Vec3::splat(f32::NEG_INFINITY);
    for v in &model.vertices {
        let p = Vec3::from(v.pos);
        min_b = min_b.min(p);
        max_b = max_b.max(p);
    }
    (min_b, max_b)
}

fn compute_bounds_basic(model: &CpuMesh) -> (Vec3, Vec3) {
    let mut min_b = Vec3::splat(f32::INFINITY);
    let mut max_b = Vec3::splat(f32::NEG_INFINITY);
    for v in &model.vertices {
        let p = Vec3::from(v.pos);
        min_b = min_b.min(p);
        max_b = max_b.max(p);
    }
    (min_b, max_b)
}

fn create_depth(
    device: &wgpu::Device,
    w: u32,
    h: u32,
    fmt: wgpu::TextureFormat,
) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth"),
        size: wgpu::Extent3d {
            width: w.max(1),
            height: h.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: fmt,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    tex.create_view(&wgpu::TextureViewDescriptor::default())
}

fn scale_to_max((w, h): (u32, u32), max_dim: u32) -> (u32, u32) {
    if w <= max_dim && h <= max_dim {
        return (w, h);
    }
    let aspect = (w as f32) / (h as f32);
    if w >= h {
        let nw = max_dim;
        let nh = (max_dim as f32 / aspect).round().max(1.0) as u32;
        (nw, nh)
    } else {
        let nh = max_dim;
        let nw = (max_dim as f32 * aspect).round().max(1.0) as u32;
        (nw, nh)
    }
}
