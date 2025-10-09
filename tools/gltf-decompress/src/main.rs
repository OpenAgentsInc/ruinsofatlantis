use std::{env, ffi::OsStr, fs, path::PathBuf, process::Command};

use anyhow::{Context, Result};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use roa_assets::gltf::load_gltf_mesh;

fn main() {
    let mut args = env::args_os().skip(1);
    let Some(input) = args.next() else {
        eprintln!("Missing <input.gltf>");
        std::process::exit(2)
    };
    let Some(output) = args.next() else {
        eprintln!("Missing <output.gltf>");
        std::process::exit(2)
    };

    let input = PathBuf::from(input);
    let output = PathBuf::from(output);

    let candidates: Vec<Vec<&OsStr>> = vec![
        vec![
            OsStr::new("@gltf-transform/cli"),
            OsStr::new("draco"),
            OsStr::new("-d"),
            input.as_os_str(),
            output.as_os_str(),
        ],
        vec![
            OsStr::new("gltf-transform"),
            OsStr::new("draco"),
            OsStr::new("-d"),
            input.as_os_str(),
            output.as_os_str(),
        ],
        vec![
            OsStr::new("@gltf-transform/cli"),
            OsStr::new("decompress"),
            input.as_os_str(),
            output.as_os_str(),
        ],
        vec![
            OsStr::new("gltf-transform"),
            OsStr::new("decompress"),
            input.as_os_str(),
            output.as_os_str(),
        ],
    ];

    // Try npx
    if let Ok(npx) = which::which("npx") {
        for args in &candidates {
            if run(npx.as_os_str(), Some(OsStr::new("-y")), args) {
                return;
            }
        }
    }
    // Try global
    for args in &candidates {
        if run(args[0], None, &args[1..]) {
            return;
        }
    }

    // Fallback: native decode ruins (Draco) → minimal glTF JSON (pos+normals+indices)
    if let Err(e) = native_decompress(&input, &output) {
        eprintln!(
            "Failed to run gltf-transform and native fallback failed: {e}\nInstall Node and try:\n  npx -y @gltf-transform/cli draco -d {} {}",
            input.display(),
            output.display()
        );
        std::process::exit(1);
    }
}

fn run(cmd: &std::ffi::OsStr, first: Option<&OsStr>, rest: &[&OsStr]) -> bool {
    let mut c = Command::new(cmd);
    if let Some(f) = first {
        c.arg(f);
    }
    let status = c.args(rest).status();
    match status {
        Ok(s) if s.success() => {
            println!("OK");
            true
        }
        Ok(s) => {
            eprintln!("{} exited with status {s}", cmd.to_string_lossy());
            false
        }
        Err(e) => {
            eprintln!("{} failed: {e}", cmd.to_string_lossy());
            false
        }
    }
}

use std::path::Path;

fn native_decompress(input: &Path, output: &Path) -> Result<()> {
    let mesh = load_gltf_mesh(input)
        .with_context(|| format!("decode Draco from {} (native)", input.display()))?;
    // Flatten into a single interleaved buffer: positions, normals, then indices (u16)
    let mut pos_min = [f32::INFINITY; 3];
    let mut pos_max = [f32::NEG_INFINITY; 3];
    for v in &mesh.vertices {
        pos_min[0] = pos_min[0].min(v.pos[0]);
        pos_min[1] = pos_min[1].min(v.pos[1]);
        pos_min[2] = pos_min[2].min(v.pos[2]);
        pos_max[0] = pos_max[0].max(v.pos[0]);
        pos_max[1] = pos_max[1].max(v.pos[1]);
        pos_max[2] = pos_max[2].max(v.pos[2]);
    }
    let mut buf: Vec<u8> = Vec::new();
    let pos_off = 0usize;
    for v in &mesh.vertices {
        buf.extend_from_slice(&v.pos[0].to_le_bytes());
        buf.extend_from_slice(&v.pos[1].to_le_bytes());
        buf.extend_from_slice(&v.pos[2].to_le_bytes());
    }
    let nrm_off = buf.len();
    for v in &mesh.vertices {
        buf.extend_from_slice(&v.nrm[0].to_le_bytes());
        buf.extend_from_slice(&v.nrm[1].to_le_bytes());
        buf.extend_from_slice(&v.nrm[2].to_le_bytes());
    }
    let idx_off = buf.len();
    for i in &mesh.indices {
        buf.extend_from_slice(&i.to_le_bytes());
    }
    let byte_length = buf.len();
    let data_uri = format!(
        "data:application/octet-stream;base64,{}",
        BASE64.encode(&buf)
    );
    let vcount = mesh.vertices.len() as u32;
    let icount = mesh.indices.len() as u32;

    let json = serde_json::json!({
        "asset": { "version": "2.0" },
        "buffers": [ { "byteLength": byte_length, "uri": data_uri } ],
        "bufferViews": [
            {"buffer": 0, "byteOffset": pos_off, "byteLength": 12 * vcount as usize, "target": 34962},
            {"buffer": 0, "byteOffset": nrm_off, "byteLength": 12 * vcount as usize, "target": 34962},
            {"buffer": 0, "byteOffset": idx_off, "byteLength": 2 * icount as usize, "target": 34963}
        ],
        "accessors": [
            {"bufferView": 0, "componentType": 5126, "count": vcount, "type": "VEC3", "min": pos_min, "max": pos_max},
            {"bufferView": 1, "componentType": 5126, "count": vcount, "type": "VEC3"},
            {"bufferView": 2, "componentType": 5123, "count": icount, "type": "SCALAR"}
        ],
        "meshes": [ {"primitives": [ {"attributes": {"POSITION": 0, "NORMAL": 1}, "indices": 2} ]} ],
        "nodes": [ {"mesh": 0} ],
        "scenes": [ {"nodes": [0]} ],
        "scene": 0
    });
    let text = serde_json::to_string_pretty(&json)?;
    fs::write(output, text).with_context(|| format!("write {}", output.display()))?;
    println!("Wrote decompressed glTF to {}", output.display());
    Ok(())
}
