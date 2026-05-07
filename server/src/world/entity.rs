use std::collections::HashMap;

use protocol::Coord;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntityType {
    Player,
    Zombie,
    Skeleton,
    Goblin,
    Rat,
}

impl EntityType {
    pub fn to_u8(&self) -> u8 {
        match self {
            EntityType::Player => 0,
            EntityType::Zombie => 1,
            EntityType::Skeleton => 2,
            EntityType::Goblin => 3,
            EntityType::Rat => 4,
        }
    }
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
    entities: HashMap<u32, Entity>,
    next_id: u32,
}

impl EntityManager {
    pub fn new() -> Self {
        Self {
            entities: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn spawn(&mut self, name: String, entity_type: EntityType, pos: Coord, hp: i32) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.entities.insert(
            id,
            Entity {
                id,
                name,
                entity_type,
                pos,
                hp,
                max_hp: hp,
            },
        );
        id
    }

    pub fn get(&self, id: u32) -> Option<&Entity> {
        self.entities.get(&id)
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut Entity> {
        self.entities.get_mut(&id)
    }

    pub fn remove(&mut self, id: u32) -> Option<Entity> {
        self.entities.remove(&id)
    }

    pub fn move_entity(&mut self, id: u32, new_pos: Coord) -> bool {
        if let Some(entity) = self.entities.get_mut(&id) {
            entity.pos = new_pos;
            true
        } else {
            false
        }
    }

    pub fn entities_in_radius(&self, cx: i32, cy: i32, radius: i32) -> Vec<&Entity> {
        self.entities
            .values()
            .filter(|e| (e.pos.x - cx).abs() <= radius && (e.pos.y - cy).abs() <= radius)
            .collect()
    }

    pub fn all(&self) -> impl Iterator<Item = &Entity> {
        self.entities.values()
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }
}
