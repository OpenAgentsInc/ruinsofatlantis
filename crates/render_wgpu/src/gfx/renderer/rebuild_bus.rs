//! Resize/Rebuild bus: central place to notify subsystems when attachments
//! or surface configuration change so they can rebuild bind groups or state.

#[allow(clippy::type_complexity)]
pub struct RebuildBusCore<T> {
    listeners: Vec<Box<dyn Fn(&mut T) + Send + Sync>>,
}

impl<T> RebuildBusCore<T> {
    pub fn new() -> Self {
        Self {
            listeners: Vec::new(),
        }
    }
    pub fn register<F>(&mut self, f: F)
    where
        F: Fn(&mut T) + Send + Sync + 'static,
    {
        self.listeners.push(Box::new(f));
    }
    pub fn run_all(&self, t: &mut T) {
        for cb in &self.listeners {
            cb(t);
        }
    }
}

pub type RebuildBus = RebuildBusCore<crate::gfx::Renderer>;

#[cfg(test)]
mod core_tests {
    use super::RebuildBusCore;
    #[test]
    fn listeners_run_in_order() {
        let mut bus: RebuildBusCore<u32> = RebuildBusCore::new();
        bus.register(|t| *t *= 10);
        bus.register(|t| *t += 3);
        let mut v = 1u32;
        bus.run_all(&mut v);
        assert_eq!(v, 13);
    }
}
