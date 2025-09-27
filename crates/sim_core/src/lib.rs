//! sim_core: rules + combat + sim facade.
//!
//! Temporary re-exports of the root crate's core rules/combat and sim APIs
//! so other crates can begin to depend on these paths.

pub use ruinsofatlantis::core::{combat, rules};
pub use ruinsofatlantis::sim;
