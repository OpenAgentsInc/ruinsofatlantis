// Re-export production modules for use by multiple binaries.
pub mod assets;
pub use client_core as client;
pub use ecs_core as ecs;
pub mod gfx;
pub use platform_winit;
pub use server_core as server;
pub use sim_core as sim;
