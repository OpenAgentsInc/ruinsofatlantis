//! Client runtime systems: input, player controllers, and camera follow.
//!
//! This module hosts lightweight client-side logic that ties platform input
//! to in-world character movement and camera control. Keep these systems
//! focused, documented, and decoupled from rendering and simulation.

pub mod input;
pub mod controller;
