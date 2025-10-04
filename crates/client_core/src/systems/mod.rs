//! Client-side systems for input/controller and camera.
//!
//! Hosts lightweight, testable logic used by the renderer host.

pub mod action_bindings;
pub mod camera;
pub mod cursor;
pub mod mouselook;

#[cfg(test)]
mod tests {
    #[test]
    fn systems_placeholder_runs() {
        assert_eq!(1 + 1, 2);
    }
}
