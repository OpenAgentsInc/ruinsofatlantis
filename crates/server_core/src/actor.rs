//! Authoritative actor store and basic types (pre-ECS bridge).

use glam::Vec3;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ActorId(pub u32);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ActorKind {
    Wizard,
    Zombie,
    Boss,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Team {
    Pc,
    Wizards,
    Undead,
    Neutral,
}

#[derive(Copy, Clone, Debug)]
pub struct Health {
    pub hp: i32,
    pub max: i32,
}
impl Health {
    #[inline]
    pub fn alive(&self) -> bool { self.hp > 0 }
    #[inline]
    pub fn clamp(&mut self) { if self.hp > self.max { self.hp = self.max; } }
}

#[derive(Copy, Clone, Debug)]
pub struct Transform {
    pub pos: Vec3,
    pub yaw: f32,
    pub radius: f32,
}

#[derive(Copy, Clone, Debug)]
pub struct Actor {
    pub id: ActorId,
    pub kind: ActorKind,
    pub team: Team,
    pub tr: Transform,
    pub hp: Health,
}

#[derive(Default, Debug)]
pub struct ActorStore {
    next_id: u32,
    pub actors: Vec<Actor>,
}

impl ActorStore {
    pub fn spawn(&mut self, kind: ActorKind, team: Team, tr: Transform, hp: Health) -> ActorId {
        let id = ActorId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        self.actors.push(Actor { id, kind, team, tr, hp });
        id
    }

    #[inline]
    pub fn get(&self, id: ActorId) -> Option<&Actor> { self.actors.iter().find(|a| a.id == id) }
    #[inline]
    pub fn get_mut(&mut self, id: ActorId) -> Option<&mut Actor> { self.actors.iter_mut().find(|a| a.id == id) }
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item=&Actor> { self.actors.iter() }
    #[inline]
    pub fn iter_mut(&mut self) -> impl Iterator<Item=&mut Actor> { self.actors.iter_mut() }

    pub fn remove_dead(&mut self) {
        self.actors.retain(|a| a.hp.alive());
    }
}
