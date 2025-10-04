//! Simple in-proc channel for replication messages (bytes).
//!
//! This is intentionally minimal for the local loop in 95I. It uses
//! `std::sync::mpsc` under the hood and exposes non-blocking drain helpers.

use std::sync::mpsc::{self, Receiver, Sender};

#[derive(Clone)]
pub struct Tx(pub Sender<Vec<u8>>);
pub struct Rx(pub Receiver<Vec<u8>>);

/// Create a sender/receiver pair. The underlying channel is unbounded.
#[must_use]
pub fn channel() -> (Tx, Rx) {
    let (s, r) = mpsc::channel::<Vec<u8>>();
    (Tx(s), Rx(r))
}

impl Tx {
    /// Try to send; returns false if the receiver is dropped.
    #[must_use]
    pub fn try_send(&self, bytes: Vec<u8>) -> bool {
        self.0.send(bytes).is_ok()
    }
}

impl Rx {
    /// Non-blocking receive of a single message.
    #[must_use]
    pub fn try_recv(&self) -> Option<Vec<u8>> {
        self.0.try_recv().ok()
    }
    /// Drain all currently queued messages.
    #[must_use]
    pub fn drain(&self) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        while let Some(b) = self.try_recv() {
            out.push(b);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn send_and_drain() {
        let (tx, rx) = channel();
        assert!(tx.try_send(vec![1, 2, 3]));
        assert!(tx.try_send(vec![4, 5]));
        let drained = rx.drain();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0], vec![1, 2, 3]);
    }
}
