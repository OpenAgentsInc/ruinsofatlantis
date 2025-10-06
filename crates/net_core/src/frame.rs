//! Simple versioned length framing for snapshot messages.
//!
//! Format (little-endian):
//! - u8 `FRAME_VERSION` (1)
//! - u32 LEN (bytes of payload)
//! - [u8; LEN] payload
//!
//! This allows multiplexed streams to delimit messages without peeking into
//! inner payloads. Inner payloads may themselves be versioned.

const FRAME_VERSION: u8 = 1;
const MAX_FRAME_LEN: usize = 1_048_576; // 1 MiB cap for safety

/// Write a framed message into `out`, appending to any existing bytes.
pub fn write_msg(out: &mut Vec<u8>, payload: &[u8]) {
    out.push(FRAME_VERSION);
    let len = u32::try_from(payload.len()).unwrap_or(0);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(payload);
}

/// Read a single framed message from `inp`. Returns the payload slice on success.
///
/// The returned slice borrows from `inp` and is valid as long as `inp` is.
pub fn read_msg(inp: &[u8]) -> anyhow::Result<&[u8]> {
    use anyhow::bail;
    if inp.len() < 5 {
        bail!("short frame header");
    }
    let ver = inp[0];
    if ver != FRAME_VERSION {
        bail!("unsupported frame version: {ver}");
    }
    let mut lenb = [0u8; 4];
    lenb.copy_from_slice(&inp[1..5]);
    let len = u32::from_le_bytes(lenb) as usize;
    if len > MAX_FRAME_LEN {
        bail!("frame too large: {len} > {MAX_FRAME_LEN}");
    }
    if inp.len() < 5 + len {
        bail!("short frame payload");
    }
    Ok(&inp[5..5 + len])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_frame() {
        let payload = b"hello";
        let mut buf = Vec::new();
        write_msg(&mut buf, payload);
        let got = read_msg(&buf).expect("read");
        assert_eq!(got, payload);
    }
    #[test]
    fn rejects_wrong_version() {
        let mut buf = vec![2u8, 0, 0, 0, 0];
        assert!(read_msg(&buf).is_err());
        // fix version but declare oversize to trigger cap
        buf[0] = FRAME_VERSION;
        buf[1..5].copy_from_slice(&(u32::MAX).to_le_bytes());
        assert!(read_msg(&buf).is_err());
    }
}
