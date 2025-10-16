//! Wishcraft core crate.
//!
//! Scope: schema models, linting, scoring, heat estimation, registry traits,
//! shadow-run/execute traits, and ledger entry types. Pure and dependency-light.

pub mod conduit;
pub mod execute;
pub mod heat;
pub mod ledger;
pub mod lint;
pub mod registry;
pub mod schema;
pub mod score;
pub mod shadow;

pub use heat::{HeatBreakdown, estimate_heat, estimate_heat_with_conduits};
pub use lint::{LintReport, lint_wish};
pub use schema::{Budget, Scope, Tier, Wish};
pub use score::{Scores, score_wish};
