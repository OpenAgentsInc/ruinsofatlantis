use anyhow::{Context, Result, anyhow};
use clap::Parser;
use serde::Serialize;
use std::path::{Path, PathBuf};

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
}

#[derive(Serialize)]
struct TextureDto {
    width: u32,
    height: u32,
    srgb: bool,
    /// RGBA8 pixels, base64-encoded
    data_b64: String,
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
    let submeshes = cpu
        .submeshes
        .iter()
        .map(|sm| SubmeshDto {
            start: sm.start,
            count: sm.count,
            base_color: sm.base_color_texture.as_ref().map(|t| TextureDto {
                width: t.width,
                height: t.height,
                srgb: t.srgb,
                data_b64: base64::encode(&t.pixels),
            }),
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
