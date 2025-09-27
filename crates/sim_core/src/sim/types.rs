//! Small shared types for sim runtime.

#[derive(Copy, Clone, Debug)]
pub enum Outcome {
    Hit,
    Miss,
    Crit,
}
