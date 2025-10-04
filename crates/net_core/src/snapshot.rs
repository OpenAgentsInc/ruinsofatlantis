//! Snapshot encode/decode traits and stub message types for replication.
//!
//! This module intentionally stays simple for v0; later phases can swap in
//! better encoders or deltas without breaking clients of these traits.

/// Types implementing snapshot encoding write themselves into a byte buffer.
pub trait SnapshotEncode {
    fn encode(&self, out: &mut Vec<u8>);
}

/// Types implementing snapshot decoding reconstruct themselves from a byte slice.
pub trait SnapshotDecode: Sized {
    fn decode(inp: &mut &[u8]) -> anyhow::Result<Self>;
}

/// Minimal entity header stub for per-entity records in a snapshot stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityHeader {
    pub id: u64,
    pub archetype: u16,
}

/// A compact CPU mesh delta for a single voxel chunk.
#[derive(Debug, Clone, PartialEq)]
pub struct ChunkMeshDelta {
    pub did: u64,
    pub chunk: (u32, u32, u32),
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

impl SnapshotEncode for ChunkMeshDelta {
    fn encode(&self, out: &mut Vec<u8>) {
        // naive encoding (for scaffold only)
        out.extend_from_slice(&self.did.to_le_bytes());
        out.extend_from_slice(&self.chunk.0.to_le_bytes());
        out.extend_from_slice(&self.chunk.1.to_le_bytes());
        out.extend_from_slice(&self.chunk.2.to_le_bytes());
        let npos = u32::try_from(self.positions.len()).expect("positions len fits u32");
        out.extend_from_slice(&npos.to_le_bytes());
        for p in &self.positions {
            for c in p {
                out.extend_from_slice(&c.to_le_bytes());
            }
        }
        let nnrm = u32::try_from(self.normals.len()).expect("normals len fits u32");
        out.extend_from_slice(&nnrm.to_le_bytes());
        for n in &self.normals {
            for c in n {
                out.extend_from_slice(&c.to_le_bytes());
            }
        }
        let nidx = u32::try_from(self.indices.len()).expect("indices len fits u32");
        out.extend_from_slice(&nidx.to_le_bytes());
        for i in &self.indices {
            out.extend_from_slice(&i.to_le_bytes());
        }
    }
}

impl SnapshotDecode for ChunkMeshDelta {
    fn decode(inp: &mut &[u8]) -> anyhow::Result<Self> {
        use anyhow::bail;
        fn take<const N: usize>(inp: &mut &[u8]) -> anyhow::Result<[u8; N]> {
            if inp.len() < N {
                anyhow::bail!("short read");
            }
            let (a, b) = inp.split_at(N);
            *inp = b;
            let mut buf = [0u8; N];
            buf.copy_from_slice(a);
            Ok(buf)
        }
        let did = u64::from_le_bytes(take::<8>(inp)?);
        let cx = u32::from_le_bytes(take::<4>(inp)?);
        let cy = u32::from_le_bytes(take::<4>(inp)?);
        let cz = u32::from_le_bytes(take::<4>(inp)?);
        let npos = u32::from_le_bytes(take::<4>(inp)?) as usize;
        let mut positions = Vec::with_capacity(npos);
        for _ in 0..npos {
            positions.push([
                f32::from_le_bytes(take::<4>(inp)?),
                f32::from_le_bytes(take::<4>(inp)?),
                f32::from_le_bytes(take::<4>(inp)?),
            ]);
        }
        let nnrm = u32::from_le_bytes(take::<4>(inp)?) as usize;
        let mut normals = Vec::with_capacity(nnrm);
        for _ in 0..nnrm {
            normals.push([
                f32::from_le_bytes(take::<4>(inp)?),
                f32::from_le_bytes(take::<4>(inp)?),
                f32::from_le_bytes(take::<4>(inp)?),
            ]);
        }
        let nidx = u32::from_le_bytes(take::<4>(inp)?) as usize;
        let mut indices = Vec::with_capacity(nidx);
        for _ in 0..nidx {
            indices.push(u32::from_le_bytes(take::<4>(inp)?));
        }
        if positions.len() != normals.len() {
            bail!(
                "positions/normals length mismatch: {}/{}",
                positions.len(),
                normals.len()
            );
        }
        Ok(Self {
            did,
            chunk: (cx, cy, cz),
            positions,
            normals,
            indices,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delta_roundtrip_naive() {
        let d = ChunkMeshDelta {
            did: 1,
            chunk: (2, 3, 4),
            positions: vec![[0.0, 1.0, 2.0], [3.0, 4.0, 5.0]],
            normals: vec![[0.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
            indices: vec![0, 1, 2],
        };
        let mut buf = Vec::new();
        d.encode(&mut buf);
        let mut slice: &[u8] = &buf;
        let d2 = ChunkMeshDelta::decode(&mut slice).expect("decode");
        assert_eq!(d.did, d2.did);
        assert_eq!(d.chunk, d2.chunk);
        assert_eq!(d.positions.len(), d2.positions.len());
        assert_eq!(d.indices, d2.indices);
    }
}
