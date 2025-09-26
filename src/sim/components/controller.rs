//! Controller marker (player input or AI policy).

#[derive(Copy, Clone, Debug)]
pub enum Controller {
    Player,
    AI,
}
