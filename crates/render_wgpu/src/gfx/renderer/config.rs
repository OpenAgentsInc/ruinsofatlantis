#![allow(dead_code)]
//! Renderer configuration toggles & constants (scaffolding).
//!
//! This module centralizes small constants and feature toggles used by the
//! renderer. In phase one, it serves as a home for values currently spread
//! across init/mod; later passes can migrate usage here.

/// Default maximum render target dimension (clamped from window size).
pub const DEFAULT_MAX_DIM: u32 = 4096;

/// Default BindGroup cache capacity; sized to avoid churn across post/HUD variants.
pub const BG_CACHE_CAP: usize = 4096;

/// Enable post ambient-occlusion pass by default.
pub const DEFAULT_ENABLE_POST_AO: bool = true;
/// Enable SSGI pass by default.
pub const DEFAULT_ENABLE_SSGI: bool = false;
/// Enable SSR pass by default.
pub const DEFAULT_ENABLE_SSR: bool = false;
/// Enable bloom pass by default.
pub const DEFAULT_ENABLE_BLOOM: bool = true;
