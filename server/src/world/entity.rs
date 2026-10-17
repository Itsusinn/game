use protocol::Coord;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityType {
    Player,
    Zombie,
    Skeleton,
    Goblin,
    Rat,
}
impl EntityType {
    pub fn to_u8(self) -> u8 {
        self as u8
    }
}

// Simulation components. Network IDs remain independent of hecs generations.
#[derive(Debug, Clone)]
pub struct Identity {
    pub id: u32,
    pub name: String,
    pub kind: EntityType,
}
#[derive(Debug, Clone, Copy)]
pub struct Position(pub Coord);
#[derive(Debug, Clone, Copy)]
pub struct Health {
    pub current: i32,
    pub maximum: i32,
}
#[derive(Debug, Clone)]
pub struct Entity {
    pub id: u32,
    pub name: String,
    pub entity_type: EntityType,
    pub pos: Coord,
    pub hp: i32,
    pub max_hp: i32,
}

pub struct EntityManager {
    world: hecs::World,
    ids: HashMap<u32, hecs::Entity>,
    next_id: u32,
}
impl Default for EntityManager {
    fn default() -> Self {
        Self::new()
    }
}
impl EntityManager {
    pub fn new() -> Self {
        Self {
            world: hecs::World::new(),
            ids: HashMap::new(),
            next_id: 1 << 31,
        }
    }
    fn insert(&mut self, id: u32, name: String, kind: EntityType, pos: Coord, hp: i32) {
        self.remove(id);
        let entity = self.world.spawn((
            Identity { id, name, kind },
            Position(pos),
            Health {
                current: hp,
                maximum: hp,
            },
        ));
        self.ids.insert(id, entity);
    }
    pub fn spawn(&mut self, name: String, kind: EntityType, pos: Coord, hp: i32) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1).expect("NPC ID space exhausted");
        self.insert(id, name, kind, pos, hp);
        id
    }
    pub fn spawn_player(&mut self, id: u32, pos: Coord, hp: i32) {
        assert!(id < 1 << 31, "player ID outside reserved range");
        self.insert(id, format!("Player_{id}"), EntityType::Player, pos, hp);
    }
    pub fn get(&self, id: u32) -> Option<Entity> {
        let handle = *self.ids.get(&id)?;
        let identity = self.world.get::<&Identity>(handle).ok()?;
        let pos = self.world.get::<&Position>(handle).ok()?;
        let health = self.world.get::<&Health>(handle).ok()?;
        Some(Entity {
            id,
            name: identity.name.clone(),
            entity_type: identity.kind,
            pos: pos.0,
            hp: health.current,
            max_hp: health.maximum,
        })
    }
    pub fn set_hp(&mut self, id: u32, hp: i32) {
        if let Some(handle) = self.ids.get(&id) {
            if let Ok(mut health) = self.world.get::<&mut Health>(*handle) {
                health.current = hp.max(0);
            }
        }
    }
    pub fn remove(&mut self, id: u32) -> Option<Entity> {
        let snapshot = self.get(id)?;
        let handle = self.ids.remove(&id)?;
        let _ = self.world.despawn(handle);
        Some(snapshot)
    }
    pub fn move_entity(&mut self, id: u32, new_pos: Coord) -> bool {
        if let Some(handle) = self.ids.get(&id) {
            if let Ok(mut pos) = self.world.get::<&mut Position>(*handle) {
                pos.0 = new_pos;
                return true;
            }
        }
        false
    }
    pub fn entities_in_radius(&self, x: i32, y: i32, radius: i32) -> Vec<Entity> {
        self.world
            .query::<(&Identity, &Position)>()
            .iter()
            .filter(|(_, p)| (p.0.x - x).abs() <= radius && (p.0.y - y).abs() <= radius)
            .filter_map(|(id, _)| self.get(id.id))
            .collect()
    }
    pub fn all(&self) -> impl Iterator<Item = Entity> {
        let mut result: Vec<_> = self
            .world
            .query::<(&Identity, &Position, &Health)>()
            .iter()
            .map(|(id, pos, hp)| Entity {
                id: id.id,
                name: id.name.clone(),
                entity_type: id.kind,
                pos: pos.0,
                hp: hp.current,
                max_hp: hp.maximum,
            })
            .collect();
        result.sort_unstable_by_key(|e| e.id);
        result.into_iter()
    }
    pub fn len(&self) -> usize {
        self.ids.len()
    }
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn player_ids_never_alias_generated_enemies_or_reused_ecs_slots() {
        let mut entities = EntityManager::new();
        let enemy = entities.spawn("rat".into(), EntityType::Rat, Coord::new(0, 0), 4);
        entities.spawn_player(1, Coord::new(2, 2), 100);
        entities.move_entity(1, Coord::new(3, 2));
        assert_eq!(entities.get(enemy).unwrap().pos, Coord::new(0, 0));
        entities.remove(1);
        entities.spawn_player(2, Coord::new(4, 4), 100);
        assert!(entities.get(1).is_none());
        assert_eq!(entities.get(2).unwrap().entity_type, EntityType::Player);
        assert_eq!(entities.len(), 2);
    }
}
