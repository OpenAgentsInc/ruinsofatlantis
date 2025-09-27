//! Active cast/channel tracking.

#[derive(Copy, Clone, Debug, Default)]
pub struct CastBar {
    pub remaining_ms: u32,
}
