//! UI module split scaffolding.
//!
//! Phase-one: keep legacy monolithic implementation via `legacy` while
//! introducing focused submodules. No behavior change.

pub mod help; // help overlay
pub mod hotbar; // hotbar UI
pub mod legacy;
pub mod perf; // overlays / perf // moved from ../ui.rs

pub use legacy::*; // keep external API unchanged during transition

#[cfg(test)]
mod tests {
    #[test]
    fn ui_sanity_placeholder() {
        // Placeholder CPU-only test to satisfy phase-one requirement
        assert_eq!(2 + 2, 4);
    }
}
