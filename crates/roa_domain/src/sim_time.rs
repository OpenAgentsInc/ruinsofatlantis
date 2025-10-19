//! Simple simulation time resource for fixed-step logic.
use bevy_ecs::prelude::*;
use bevy_time::Time;

#[derive(Resource, Debug, Clone, Copy)]
pub struct SimTime {
    pub tick: u64,
    pub dt: f32, // seconds
}

impl Default for SimTime {
    fn default() -> Self {
        Self {
            tick: 0,
            dt: 1.0 / 60.0,
        }
    }
}

/// Update the simulation time each tick. Intended for the FixedUpdate schedule.
pub fn tick_sim_time(mut sim: ResMut<SimTime>, time: Res<Time>) {
    sim.tick = sim.tick.saturating_add(1);
    // Use the delta corresponding to the current schedule (fixed if running in FixedUpdate).
    sim.dt = time.delta_secs();
}
