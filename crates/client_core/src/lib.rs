//! Client glue: input state and a simple third‑person controller.

pub mod input {
    #[derive(Default, Debug, Clone, Copy)]
    pub struct InputState {
        pub forward: bool,
        pub backward: bool,
        pub left: bool,
        pub right: bool,
        pub run: bool, // Shift
    }
    impl InputState {
        pub fn clear(&mut self) {
            *self = Self::default();
        }
    }
}

pub mod controller {
    use super::input::InputState;
    use glam::Vec3;

    #[derive(Debug, Clone, Copy)]
    pub struct PlayerController {
        pub pos: Vec3,
        pub yaw: f32,
    }
    impl PlayerController {
        pub fn new(initial_pos: Vec3) -> Self {
            Self {
                pos: initial_pos,
                yaw: 0.0,
            }
        }
        pub fn update(&mut self, input: &InputState, dt: f32, _cam_forward: Vec3) {
            let speed = if input.run { 9.0 } else { 5.0 };
            let yaw_rate = 1.8; // rad/s
            let only_backward = input.backward && !input.left && !input.right && !input.forward;
            if !only_backward {
                if input.left {
                    self.yaw = wrap_angle(self.yaw + yaw_rate * dt);
                }
                if input.right {
                    self.yaw = wrap_angle(self.yaw - yaw_rate * dt);
                }
            }
            let fwd = Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos()).normalize_or_zero();
            if input.forward && !input.backward {
                self.pos += fwd * speed * dt;
            } else if input.backward && !input.forward {
                self.pos -= fwd * speed * dt;
            }
        }
    }
    fn wrap_angle(a: f32) -> f32 {
        let mut x = a;
        while x > std::f32::consts::PI {
            x -= std::f32::consts::TAU;
        }
        while x < -std::f32::consts::PI {
            x += std::f32::consts::TAU;
        }
        x
    }
}
