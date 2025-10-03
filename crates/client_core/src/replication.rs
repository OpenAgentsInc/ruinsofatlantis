//! Client replication scaffolding.
//!
//! Responsibilities
//! - Buffer incoming snapshot deltas
//! - Apply to client ECS/state
//! - Invalidate GPU uploads for changed chunks
//!
//! Filled in later when net_core types are finalized.

/// Opaque replication buffer (placeholder).
#[derive(Default, Debug)]
pub struct ReplicationBuffer {
    pub updated_chunks: usize,
}

impl ReplicationBuffer {
    /// Apply a raw message. Returns whether any state changed.
    pub fn apply_message(&mut self, _bytes: &[u8]) -> bool {
        // TODO: parse via net_core snapshot decode types
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn buffer_default_is_empty() {
        let b = ReplicationBuffer::default();
        assert_eq!(b.updated_chunks, 0);
    }
}
