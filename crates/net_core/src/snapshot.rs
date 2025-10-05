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

const VERSION: u8 = 1;
const MAX_MESH_ELEMS: usize = 262_144; // conservative cap to prevent OOM

impl SnapshotEncode for ChunkMeshDelta {
    fn encode(&self, out: &mut Vec<u8>) {
        // versioned encoding (scaffold)
        out.push(VERSION);
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
        let ver = inp
            .first()
            .copied()
            .ok_or_else(|| anyhow::anyhow!("short read"))?;
        *inp = &inp[1..];
        if ver != VERSION {
            bail!("unsupported version: {ver}");
        }
        let did = u64::from_le_bytes(take::<8>(inp)?);
        let cx = u32::from_le_bytes(take::<4>(inp)?);
        let cy = u32::from_le_bytes(take::<4>(inp)?);
        let cz = u32::from_le_bytes(take::<4>(inp)?);
        let npos = u32::from_le_bytes(take::<4>(inp)?) as usize;
        if npos > MAX_MESH_ELEMS {
            bail!("npos too large: {npos}");
        }
        let mut positions = Vec::with_capacity(npos);
        for _ in 0..npos {
            positions.push([
                f32::from_le_bytes(take::<4>(inp)?),
                f32::from_le_bytes(take::<4>(inp)?),
                f32::from_le_bytes(take::<4>(inp)?),
            ]);
        }
        let nnrm = u32::from_le_bytes(take::<4>(inp)?) as usize;
        if nnrm > MAX_MESH_ELEMS {
            bail!("nnrm too large: {nnrm}");
        }
        let mut normals = Vec::with_capacity(nnrm);
        for _ in 0..nnrm {
            normals.push([
                f32::from_le_bytes(take::<4>(inp)?),
                f32::from_le_bytes(take::<4>(inp)?),
                f32::from_le_bytes(take::<4>(inp)?),
            ]);
        }
        let nidx = u32::from_le_bytes(take::<4>(inp)?) as usize;
        if nidx > MAX_MESH_ELEMS * 6 {
            bail!("nidx too large: {nidx}");
        }
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

/// Minimal replicated record for a destructible instance's world AABB.
#[derive(Debug, Clone, PartialEq)]
pub struct DestructibleInstance {
    pub did: u64,
    pub world_min: [f32; 3],
    pub world_max: [f32; 3],
}

impl SnapshotEncode for DestructibleInstance {
    fn encode(&self, out: &mut Vec<u8>) {
        out.push(VERSION);
        out.extend_from_slice(&self.did.to_le_bytes());
        for c in &self.world_min {
            out.extend_from_slice(&c.to_le_bytes());
        }
        for c in &self.world_max {
            out.extend_from_slice(&c.to_le_bytes());
        }
    }
}

impl SnapshotDecode for DestructibleInstance {
    fn decode(inp: &mut &[u8]) -> anyhow::Result<Self> {
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
        let ver = inp
            .first()
            .copied()
            .ok_or_else(|| anyhow::anyhow!("short read"))?;
        *inp = &inp[1..];
        if ver != VERSION {
            anyhow::bail!("unsupported version: {ver}");
        }
        let did = u64::from_le_bytes(take::<8>(inp)?);
        let mut world_min = [0.0f32; 3];
        for v in &mut world_min {
            *v = f32::from_le_bytes(take::<4>(inp)?);
        }
        let mut world_max = [0.0f32; 3];
        for v in &mut world_max {
            *v = f32::from_le_bytes(take::<4>(inp)?);
        }
        Ok(Self {
            did,
            world_min,
            world_max,
        })
    }
}

/// Minimal boss status snapshot for HUD/labels.
#[derive(Debug, Clone, PartialEq)]
pub struct BossStatusMsg {
    pub name: String,
    pub ac: i32,
    pub hp: i32,
    pub max: i32,
    pub pos: [f32; 3],
}

impl SnapshotEncode for BossStatusMsg {
    fn encode(&self, out: &mut Vec<u8>) {
        out.push(VERSION);
        let n = u16::try_from(self.name.len()).unwrap_or(0);
        out.extend_from_slice(&n.to_le_bytes());
        out.extend_from_slice(self.name.as_bytes());
        out.extend_from_slice(&self.ac.to_le_bytes());
        out.extend_from_slice(&self.hp.to_le_bytes());
        out.extend_from_slice(&self.max.to_le_bytes());
        for c in &self.pos {
            out.extend_from_slice(&c.to_le_bytes());
        }
    }
}

impl SnapshotDecode for BossStatusMsg {
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
        let ver = inp
            .first()
            .copied()
            .ok_or_else(|| anyhow::anyhow!("short read"))?;
        *inp = &inp[1..];
        if ver != VERSION {
            bail!("unsupported version: {ver}");
        }
        let n = u16::from_le_bytes(take::<2>(inp)?) as usize;
        if inp.len() < n {
            bail!("short name");
        }
        let (name_bytes, rest) = inp.split_at(n);
        *inp = rest;
        let name = String::from_utf8(name_bytes.to_vec()).unwrap_or_default();
        let ac = i32::from_le_bytes(take::<4>(inp)?);
        let hp = i32::from_le_bytes(take::<4>(inp)?);
        let max = i32::from_le_bytes(take::<4>(inp)?);
        let mut pos = [0.0f32; 3];
        for v in &mut pos {
            *v = f32::from_le_bytes(take::<4>(inp)?);
        }
        Ok(Self {
            name,
            ac,
            hp,
            max,
            pos,
        })
    }
}

/// Compact list of NPC statuses for client UI/presentation.
#[derive(Debug, Clone, PartialEq)]
pub struct NpcListMsg {
    pub items: Vec<NpcItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NpcItem {
    pub id: u32,
    pub hp: i32,
    pub max: i32,
    pub pos: [f32; 3],
    pub radius: f32,
    pub alive: u8,
    pub attack_anim: f32,
}

impl SnapshotEncode for NpcListMsg {
    fn encode(&self, out: &mut Vec<u8>) {
        out.push(VERSION);
        let n = u16::try_from(self.items.len()).unwrap_or(0);
        out.extend_from_slice(&n.to_le_bytes());
        for it in &self.items {
            out.extend_from_slice(&it.id.to_le_bytes());
            out.extend_from_slice(&it.hp.to_le_bytes());
            out.extend_from_slice(&it.max.to_le_bytes());
            for c in &it.pos {
                out.extend_from_slice(&c.to_le_bytes());
            }
            out.extend_from_slice(&it.radius.to_le_bytes());
            out.push(it.alive);
            out.extend_from_slice(&it.attack_anim.to_le_bytes());
        }
    }
}

impl SnapshotDecode for NpcListMsg {
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
        let ver = inp
            .first()
            .copied()
            .ok_or_else(|| anyhow::anyhow!("short read"))?;
        *inp = &inp[1..];
        if ver != VERSION {
            bail!("unsupported version: {ver}");
        }
        let n = u16::from_le_bytes(take::<2>(inp)?) as usize;
        let mut items = Vec::with_capacity(n);
        for _ in 0..n {
            let id = u32::from_le_bytes(take::<4>(inp)?);
            let hp = i32::from_le_bytes(take::<4>(inp)?);
            let max = i32::from_le_bytes(take::<4>(inp)?);
            let mut pos = [0.0f32; 3];
            for v in &mut pos {
                *v = f32::from_le_bytes(take::<4>(inp)?);
            }
            let radius = f32::from_le_bytes(take::<4>(inp)?);
            let alive = inp
                .first()
                .copied()
                .ok_or_else(|| anyhow::anyhow!("short read"))?;
            *inp = &inp[1..];
            let attack_anim = f32::from_le_bytes(take::<4>(inp)?);
            items.push(NpcItem {
                id,
                hp,
                max,
                pos,
                radius,
                alive,
                attack_anim,
            });
        }
        Ok(Self { items })
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
    #[test]
    fn destructible_instance_roundtrip() {
        let d = DestructibleInstance {
            did: 42,
            world_min: [-1.0, 0.0, -2.0],
            world_max: [3.0, 4.0, 5.0],
        };
        let mut buf = Vec::new();
        d.encode(&mut buf);
        let mut slice: &[u8] = &buf;
        let d2 = DestructibleInstance::decode(&mut slice).expect("decode");
        assert_eq!(d, d2);
    }
    #[test]
    fn boss_status_roundtrip() {
        let s = BossStatusMsg {
            name: "Nivita".into(),
            ac: 18,
            hp: 220,
            max: 250,
            pos: [1.0, 2.0, -3.0],
        };
        let mut buf = Vec::new();
        s.encode(&mut buf);
        let mut slice: &[u8] = &buf;
        let s2 = BossStatusMsg::decode(&mut slice).expect("decode");
        assert_eq!(s.name, s2.name);
        assert_eq!(s.ac, s2.ac);
        assert_eq!(s.hp, s2.hp);
        assert_eq!(s.max, s2.max);
        assert_eq!(s.pos, s2.pos);
    }
}
