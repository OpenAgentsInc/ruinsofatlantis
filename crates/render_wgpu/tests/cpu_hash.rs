use sha2::{Digest, Sha256};

#[test]
fn terrain_cpu_hash_is_stable() {
    // CPU-only generation; no GPU device needed.
    let cpu = render_wgpu::gfx::terrain::generate_cpu(129, 150.0, 12345);
    let mut hasher = Sha256::new();
    hasher.update((cpu.size as u32).to_le_bytes());
    hasher.update(cpu.extent.to_bits().to_le_bytes());
    for h in &cpu.heights {
        hasher.update(h.to_bits().to_le_bytes());
    }
    for n in &cpu.normals {
        for c in n {
            hasher.update(c.to_bits().to_le_bytes());
        }
    }
    let got = format!("{:x}", hasher.finalize());
    // Golden chosen for seed=12345, size=129, extent=150.0
    let expected = "bdcd039cf805d88d32bf9723b58e32431e546aa3a3d48d79cf78f513bf46fb99";
    assert_eq!(got, expected, "terrain cpu hash changed");
}
