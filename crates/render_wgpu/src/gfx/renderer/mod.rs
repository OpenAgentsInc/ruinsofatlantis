//! Renderer submodules: extracted from the monolithic gfx/mod.rs for clarity.
//! - passes.rs: post/overlay passes split from render()
//! - resize.rs: swapchain/attachments rebuild on window resize
//! - input.rs: window/input handling for camera + casting
//! - update.rs: CPU-side updates (AI, palettes, FX)

mod attachments;
pub mod config;
pub mod controls;
pub mod device;
pub mod graph;
pub mod init;
pub mod passes;
pub mod passes_graph;
pub mod render;
pub mod replication;
pub mod resize;
pub mod upload_adapter;
pub mod voxel_upload;
pub(crate) use attachments::Attachments;
// Re-export PcCast here so legacy update code can refer to `super::super::PcCast`.
#[allow(unused_imports)]
use crate::gfx::PcCast;
mod input;
#[path = "update/mod.rs"]
mod update;
