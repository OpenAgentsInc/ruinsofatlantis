//! Wishcraft core crate.
//!
//! Scope: schema models, linting, scoring, heat estimation, registry traits,
//! shadow-run/execute traits, and ledger entry types. Pure and dependency-light.

pub mod execute;
pub mod heat;
pub mod ledger;
pub mod lint;
pub mod registry;
pub mod schema;
pub mod score;
pub mod shadow;

pub use execute::*;
pub use heat::*;
pub use ledger::*;
pub use lint::*;
pub use registry::*;
pub use schema::*;
pub use score::*;
pub use shadow::*;
