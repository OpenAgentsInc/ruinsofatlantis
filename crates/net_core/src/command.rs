//! Client->Server commands (authoritative input/actions).
//!
//! Scope
//! - Defines a minimal binary protocol for client actions with a leading tag
//!   (`TAG_CLIENT_CMD`) distinct from `TickSnapshot` framing, so decoders can
//!   quickly reject the wrong payload type.
//! - Used by the client renderer to send cast/projectile actions to the server.
//!
//! Extending
//! - Add new enum variants (e.g., melee swings, toggles). Keep payloads small
//!   and fixed-size where possible. Versioning is handled at the message level
//!   with `TAG_CLIENT_CMD`. If the wire evolves, introduce a new tag.

use crate::snapshot::SnapshotDecode;

pub const TAG_CLIENT_CMD: u8 = 0xC1;

#[derive(Debug, Clone, PartialEq)]
pub enum ClientCmd {
    FireBolt { pos: [f32; 3], dir: [f32; 3] },
    Fireball { pos: [f32; 3], dir: [f32; 3] },
}

impl ClientCmd {
    pub fn encode(&self, out: &mut Vec<u8>) {
        out.push(TAG_CLIENT_CMD);
        match self {
            ClientCmd::FireBolt { pos, dir } => {
                out.push(0);
                for c in pos {
                    out.extend_from_slice(&c.to_le_bytes());
                }
                for c in dir {
                    out.extend_from_slice(&c.to_le_bytes());
                }
            }
            ClientCmd::Fireball { pos, dir } => {
                out.push(1);
                for c in pos {
                    out.extend_from_slice(&c.to_le_bytes());
                }
                for c in dir {
                    out.extend_from_slice(&c.to_le_bytes());
                }
            }
        }
    }
}

impl SnapshotDecode for ClientCmd {
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
        let tag = inp
            .first()
            .copied()
            .ok_or_else(|| anyhow::anyhow!("short read"))?;
        *inp = &inp[1..];
        if tag != TAG_CLIENT_CMD {
            bail!("not a client cmd tag");
        }
        let kind = inp
            .first()
            .copied()
            .ok_or_else(|| anyhow::anyhow!("short read"))?;
        *inp = &inp[1..];
        let mut pos = [0.0f32; 3];
        for v in &mut pos {
            *v = f32::from_le_bytes(take::<4>(inp)?);
        }
        let mut dir = [0.0f32; 3];
        for v in &mut dir {
            *v = f32::from_le_bytes(take::<4>(inp)?);
        }
        Ok(match kind {
            0 => Self::FireBolt { pos, dir },
            _ => Self::Fireball { pos, dir },
        })
    }
}
