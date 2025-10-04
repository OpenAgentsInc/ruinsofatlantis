//! Renderer submodules: extracted from the monolithic gfx/mod.rs for clarity.
//! - passes.rs: post/overlay passes split from render()
//! - resize.rs: swapchain/attachments rebuild on window resize
//! - input.rs: window/input handling for camera + casting
//! - update.rs: CPU-side updates (AI, palettes, FX)

mod attachments;
pub mod controls;
pub mod graph;
pub mod init;
pub mod passes;
pub mod render;
pub mod resize;
pub mod upload_adapter;
pub mod voxel_upload;
pub(crate) use attachments::Attachments;
mod input;
mod update;
