//! Core facade for data access.
//!
//! Re-exports `data_runtime` under `crate::core::data` so existing paths used
//! by the renderer continue to resolve.

pub use data_runtime as data;

