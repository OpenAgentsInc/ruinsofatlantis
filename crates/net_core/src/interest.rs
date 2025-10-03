//! Interest management scaffolding (who gets what data).
//!
//! Initial implementation will be a simple grid around the player/camera.

/// Interest providers decide whether to include an item for a given client.
pub trait InterestProvider<T> {
    fn in_interest(&self, item: &T) -> bool;
}
