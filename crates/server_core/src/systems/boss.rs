//! Boss-specific helpers (seek + integrate) for the unique Nivita NPC.

use glam::Vec3;

use crate::{NpcId, ServerState};

/// Set Nivita's velocity implicitly by moving her pos toward the nearest wizard.
/// This v0 helper operates directly on position; later we can add explicit Velocity.
pub fn boss_seek_and_integrate(state: &mut ServerState, dt: f32, wizards: &[Vec3]) {
    let Some(id) = state.nivita_id else { return; };
    let Some(n) = state.npcs.iter_mut().find(|n| n.id == id) else { return; };
    if wizards.is_empty() { return; }
    // Pick nearest wizard
    let mut best = 0usize;
    let mut best_d2 = f32::INFINITY;
    for (i, w) in wizards.iter().enumerate() {
        let dx = w.x - n.pos.x;
        let dz = w.z - n.pos.z;
        let d2 = dx * dx + dz * dz;
        if d2 < best_d2 { best_d2 = d2; best = i; }
    }
    let target = wizards[best];
    let to = Vec3::new(target.x - n.pos.x, 0.0, target.z - n.pos.z);
    let dist = to.length();
    if dist > 1e-4 {
        let step = (n.speed * dt).min(dist);
        n.pos += to.normalize() * step;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nivita_moves_toward_target() {
        let mut s = ServerState::new();
        let id = s.spawn_nivita_unique(Vec3::new(0.0, 0.6, 0.0)).expect("spawn");
        let start = s.npcs.iter().find(|n| n.id == id).unwrap().pos;
        let wizards = [Vec3::new(5.0, 0.6, 0.0)];
        boss_seek_and_integrate(&mut s, 0.5, &wizards);
        let now = s.npcs.iter().find(|n| n.id == id).unwrap().pos;
        assert!(now.x > start.x);
        let max_step = s.npcs.iter().find(|n| n.id == id).unwrap().speed * 0.5 + 1e-4;
        assert!((now.x - start.x) <= max_step);
    }
}

