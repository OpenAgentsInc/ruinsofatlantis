use anyhow::{Context, Result, anyhow};
use clap::Parser;
use serde::Serialize;
use std::path::{Path, PathBuf};

use gltf as gltf_rs;
use roa_assets::skinning::load_gltf_skinned;
use roa_assets::types::{SkinnedMeshCPU, VertexSkinCPU};
use roa_assets::util::prepare_gltf_path;

#[derive(Parser, Debug)]
#[command(name = "gltf-baker")]
#[command(about = "Minimal GLTF/GLB inspector & baker (MVP)")]
struct Cli {
    /// Input .gltf or .glb
    input: PathBuf,
    /// Output JSON summary (MVP)
    output: Option<PathBuf>,
    /// Export full SkinnedMeshCPU JSON to this path (DTO)
    #[arg(long)]
    skinned_out: Option<PathBuf>,
    /// Export animations (AnimClip DTO) to this path (JSON)
    #[arg(long)]
    anims_out: Option<PathBuf>,
}

#[derive(Serialize)]
struct Summary {
    file: String,
    scenes: usize,
    nodes: usize,
    meshes: usize,
    skins: usize,
    animations: usize,
    materials: usize,
    has_draco: bool,
}

fn scan_has_draco(doc: &gltf::Document) -> bool {
    doc.extensions_used()
        .any(|e| e == "KHR_draco_mesh_compression")
}

fn bake_summary(path: &std::path::Path) -> Result<Summary> {
    let (doc, _buffers, _images) =
        gltf::import(path).with_context(|| format!("import {}", path.display()))?;
    let sum = Summary {
        file: path.display().to_string(),
        scenes: doc.scenes().len(),
        nodes: doc.nodes().len(),
        meshes: doc.meshes().len(),
        skins: doc.skins().len(),
        animations: doc.animations().len(),
        materials: doc.materials().len(),
        has_draco: scan_has_draco(&doc),
    };
    if sum.has_draco {
        return Err(anyhow!(
            "KHR_draco_mesh_compression detected — please pre-decompress before baking"
        ));
    }
    Ok(sum)
}

#[derive(Serialize)]
struct SkinDto {
    vertices: Vec<VertDto>,
    indices: Vec<u16>,
    joints_nodes: Vec<usize>,
    inverse_bind: Vec<[f32; 16]>,
    parent: Vec<Option<usize>>,
    base_t: Vec<[f32; 3]>,
    base_r: Vec<[f32; 4]>,
    base_s: Vec<[f32; 3]>,
    submeshes: Vec<SubmeshDto>,
}

#[derive(Serialize)]
struct VertDto {
    pos: [f32; 3],
    nrm: [f32; 3],
    uv: [f32; 2],
    joints: [u16; 4],
    weights: [f32; 4],
}

#[derive(Serialize)]
struct SubmeshDto {
    start: u32,
    count: u32,
    base_color: Option<TextureDto>,
    uv_transform: Option<UvTransformDto>,
}

#[derive(Serialize)]
struct TextureDto {
    width: u32,
    height: u32,
    srgb: bool,
    /// RGBA8 pixels, base64-encoded
    data_b64: String,
}

#[derive(Serialize, Clone, Copy)]
struct UvTransformDto {
    offset: [f32; 2],
    scale: [f32; 2],
    rot: f32,
}

fn to_dto(cpu: &SkinnedMeshCPU) -> SkinDto {
    let vertices = cpu
        .vertices
        .iter()
        .map(|v: &VertexSkinCPU| VertDto {
            pos: v.pos,
            nrm: v.nrm,
            uv: v.uv,
            joints: v.joints,
            weights: v.weights,
        })
        .collect();
    let inverse_bind = cpu.inverse_bind.iter().map(|m| m.to_cols_array()).collect();
    let base_t = cpu.base_t.iter().map(|v| [v.x, v.y, v.z]).collect();
    let base_r = cpu.base_r.iter().map(|q| [q.x, q.y, q.z, q.w]).collect();
    let base_s = cpu.base_s.iter().map(|v| [v.x, v.y, v.z]).collect();
    // Submeshes + baseColor (if present)
    // Try to collect baseColor UV transforms in primitive order to match submeshes
    // Note: best-effort mapping; relies on iteration order matching loader's append order.
    let uv_transforms: Vec<Option<UvTransformDto>> = match std::env::var("GLTF_BAKER_SRC") {
        Ok(src) => match collect_basecolor_uv_transforms(Path::new(&src)) {
            Ok(v) => v,
            Err(_) => Vec::new(),
        },
        Err(_) => Vec::new(),
    };

    let submeshes = cpu
        .submeshes
        .iter()
        .enumerate()
        .map(|(i, sm)| SubmeshDto {
            start: sm.start,
            count: sm.count,
            base_color: sm.base_color_texture.as_ref().map(|t| TextureDto {
                width: t.width,
                height: t.height,
                srgb: t.srgb,
                data_b64: base64::encode(&t.pixels),
            }),
            uv_transform: uv_transforms.get(i).cloned().unwrap_or(None),
        })
        .collect();
    SkinDto {
        vertices,
        indices: cpu.indices.clone(),
        joints_nodes: cpu.joints_nodes.clone(),
        inverse_bind,
        parent: cpu.parent.clone(),
        base_t,
        base_r,
        base_s,
        submeshes,
    }
}

fn collect_basecolor_uv_transforms(path: &Path) -> Result<Vec<Option<UvTransformDto>>> {
    let (doc, buffers, _images) = gltf_rs::import(path)?;
    // Score skins by vertex count
    let mut skin_vtx: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for node in doc.nodes() {
        if let (Some(skin), Some(mesh)) = (node.skin(), node.mesh()) {
            let mut v = 0usize;
            for prim in mesh.primitives() {
                let r = prim.reader(|b| buffers.get(b.index()).map(|bb| bb.0.as_slice()));
                if let Some(pos) = r.read_positions() {
                    v += pos.size_hint().0;
                }
            }
            *skin_vtx.entry(skin.index()).or_default() += v;
        }
    }
    let best_skin = skin_vtx.into_iter().max_by_key(|(_, v)| *v).map(|(i, _)| i);
    let mut out: Vec<Option<UvTransformDto>> = Vec::new();
    if let Some(best) = best_skin {
        for node in doc.nodes() {
            if let (Some(skin), Some(mesh)) = (node.skin(), node.mesh()) {
                if skin.index() != best {
                    continue;
                }
                for prim in mesh.primitives() {
                    let pbr = prim.material().pbr_metallic_roughness();
                    let uv = pbr
                        .base_color_texture()
                        .and_then(|ti| ti.texture_transform());
                    let dto = uv.map(|t| UvTransformDto {
                        offset: [t.offset()[0] as f32, t.offset()[1] as f32],
                        scale: [t.scale()[0] as f32, t.scale()[1] as f32],
                        rot: t.rotation() as f32,
                    });
                    out.push(dto);
                }
            }
        }
    }
    Ok(out)
}

#[derive(Serialize)]
struct AnimClipsDto {
    clips: Vec<AnimClipDto>,
}

#[derive(Serialize)]
struct AnimClipDto {
    name: String,
    duration: f32,
    t_tracks: Vec<TrackVec3Dto>,
    r_tracks: Vec<TrackQuatDto>,
    s_tracks: Vec<TrackVec3Dto>,
}

#[derive(Serialize)]
struct TrackVec3Dto {
    node: usize,
    times: Vec<f32>,
    values: Vec<[f32; 3]>,
}

#[derive(Serialize)]
struct TrackQuatDto {
    node: usize,
    times: Vec<f32>,
    values: Vec<[f32; 4]>,
}

fn to_anims_dto(cpu: &SkinnedMeshCPU) -> AnimClipsDto {
    let mut clips: Vec<AnimClipDto> = Vec::new();
    // sort by name for stability
    let mut names: Vec<_> = cpu.animations.keys().cloned().collect();
    names.sort();
    for name in names {
        if let Some(clip) = cpu.animations.get(&name) {
            let mut t_tracks = Vec::new();
            for (node, tr) in clip.t_tracks.iter() {
                let values = tr.values.iter().map(|v| [v.x, v.y, v.z]).collect();
                t_tracks.push(TrackVec3Dto {
                    node: *node,
                    times: tr.times.clone(),
                    values,
                });
            }
            t_tracks.sort_by_key(|t| t.node);
            let mut r_tracks = Vec::new();
            for (node, rr) in clip.r_tracks.iter() {
                let values = rr.values.iter().map(|q| [q.x, q.y, q.z, q.w]).collect();
                r_tracks.push(TrackQuatDto {
                    node: *node,
                    times: rr.times.clone(),
                    values,
                });
            }
            r_tracks.sort_by_key(|t| t.node);
            let mut s_tracks = Vec::new();
            for (node, sr) in clip.s_tracks.iter() {
                let values = sr.values.iter().map(|v| [v.x, v.y, v.z]).collect();
                s_tracks.push(TrackVec3Dto {
                    node: *node,
                    times: sr.times.clone(),
                    values,
                });
            }
            s_tracks.sort_by_key(|t| t.node);
            clips.push(AnimClipDto {
                name: name.clone(),
                duration: clip.duration,
                t_tracks,
                r_tracks,
                s_tracks,
            });
        }
    }
    AnimClipsDto { clips }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if !cli.input.exists() {
        return Err(anyhow!("missing input: {}", cli.input.display()));
    }
    if let Some(path) = cli.skinned_out.as_ref() {
        let prepared = prepare_gltf_path(&cli.input)?;
        let cpu = load_gltf_skinned(&prepared)?;
        let dto = to_dto(&cpu);
        let data = serde_json::to_vec_pretty(&dto)?;
        std::fs::write(path, data)?;
        println!("wrote skinned DTO {}", path.display());
    } else if let Some(path) = cli.anims_out.as_ref() {
        let prepared = prepare_gltf_path(&cli.input)?;
        let cpu = load_gltf_skinned(&prepared)?;
        let dto = to_anims_dto(&cpu);
        let data = serde_json::to_vec_pretty(&dto)?;
        std::fs::write(path, data)?;
        println!("wrote anims DTO {}", path.display());
    } else {
        let sum = bake_summary(&cli.input)?;
        if let Some(out) = cli.output.as_ref() {
            let data = serde_json::to_vec_pretty(&sum)?;
            std::fs::write(out, data)?;
            println!("wrote {}", out.display());
        } else {
            println!("{}", serde_json::to_string_pretty(&sum)?);
        }
    }
    Ok(())
}
