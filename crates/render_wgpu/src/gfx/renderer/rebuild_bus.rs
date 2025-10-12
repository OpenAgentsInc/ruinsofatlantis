//! Resize/Rebuild bus: central place to notify subsystems when attachments
//! or surface configuration change so they can rebuild bind groups or state.

#[allow(clippy::type_complexity)]
pub struct RebuildBus {
    listeners: Vec<Box<dyn Fn(&mut crate::gfx::Renderer) + Send + Sync>>,
}

impl RebuildBus {
    pub fn new() -> Self {
        Self {
            listeners: Vec::new(),
        }
    }

    pub fn register<F>(&mut self, f: F)
    where
        F: Fn(&mut crate::gfx::Renderer) + Send + Sync + 'static,
    {
        self.listeners.push(Box::new(f));
    }

    pub fn run_all(&self, r: &mut crate::gfx::Renderer) {
        for cb in &self.listeners {
            cb(r);
        }
    }
}
