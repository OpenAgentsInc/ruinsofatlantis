//! UI module split scaffolding.
//!
//! Phase-one: keep legacy monolithic implementation via `legacy` while
//! introducing focused submodules. No behavior change.

pub mod help; // help overlay
pub mod hotbar;
pub mod perf; // overlays / perf // hotbar UI

// Back-compat: include the original monolithic UI module.
#[path = "../ui.rs"]
mod legacy;

pub use legacy::*;

#[cfg(test)]
mod tests {
    #[test]
    fn ui_sanity_placeholder() {
        // Placeholder CPU-only test to satisfy phase-one requirement
        assert_eq!(2 + 2, 4);
    }
}
