use glam::Vec3;

use crate::actor::{ActorId, ActorKind, Health, Team, Transform};

/// Entity handle local to this world (opaque index).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Entity(u32);

#[derive(Copy, Clone, Debug)]
pub struct Components {
    pub id: ActorId,
    pub kind: ActorKind,
    pub team: Team,
    pub tr: Transform,
    pub hp: Health,
}

#[derive(Default, Debug)]
pub struct WorldEcs {
    next_ent: u32,
    next_id: u32,
    ents: Vec<Components>,
}

impl WorldEcs {
    pub fn new() -> Self { Self::default() }

    #[inline]
    pub fn len(&self) -> usize { self.ents.len() }

    pub fn spawn(&mut self, kind: ActorKind, team: Team, tr: Transform, hp: Health) -> ActorId {
        let id = ActorId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        let _e = Entity(self.next_ent);
        self.next_ent = self.next_ent.wrapping_add(1);
        self.ents.push(Components { id, kind, team, tr, hp });
        id
    }

    pub fn get(&self, id: ActorId) -> Option<&Components> {
        self.ents.iter().find(|c| c.id == id)
    }

    pub fn get_mut(&mut self, id: ActorId) -> Option<&mut Components> {
        self.ents.iter_mut().find(|c| c.id == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Components> { self.ents.iter() }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Components> { self.ents.iter_mut() }

    pub fn remove_dead(&mut self) { self.ents.retain(|c| c.hp.alive()); }

    /// Helper: find nearest hostile actor to `pos` within optional max radius^2.
    pub fn nearest_hostile(&self, team: Team, pos: Vec3, max_r2: Option<f32>) -> Option<ActorId> {
        let mut best: Option<(f32, ActorId)> = None;
        for c in &self.ents {
            if !c.hp.alive() { continue; }
            if !hostile_default(team, c.team) { continue; }
            let dx = c.tr.pos.x - pos.x;
            let dz = c.tr.pos.z - pos.z;
            let d2 = dx * dx + dz * dz;
            if let Some(cap) = max_r2 && d2 > cap { continue; }
            if best.map(|(b, _)| d2 < b).unwrap_or(true) { best = Some((d2, c.id)); }
        }
        best.map(|(_, id)| id)
    }
}

#[inline]
fn hostile_default(a: Team, b: Team) -> bool {
    use Team::*;
    matches!((a, b), (Pc, Undead) | (Undead, Pc) | (Wizards, Undead) | (Undead, Wizards))
}
