//! Pipelines scaffolding: typed wrappers and module namespace.
//!
//! Phase-one: introduce typed wrappers without moving existing builders yet.

pub mod bloom;
pub mod common;
pub mod instanced;
pub mod post_ao;
pub mod present;
pub mod sky;
pub mod ssgi;
pub mod ssr;
pub mod terrain;

/// Grouping struct placeholder for later extraction of concrete pipelines.
#[derive(Default)]
pub struct Pipelines;
