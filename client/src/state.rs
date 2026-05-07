use protocol::*;

#[derive(Debug, Clone)]
pub struct GameState {
    pub player_id: u32,
    pub player_pos: Coord,
    pub sub_world_id: (i64, i64),
    pub world_seed: u64,
    pub visible_tiles: Vec<TileData>,
    pub explored: Vec<bool>,
    pub explored_width: u32,
    pub entities: Vec<EntityData>,
    pub items_ground: Vec<(Coord, ItemStack)>,
    pub message_log: Vec<LogEntry>,
    pub hp: i32,
    pub max_hp: i32,
    pub stamina: i32,
    pub thirst: i32,
    pub hunger: i32,
    pub seq: u32,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            player_id: 0,
            player_pos: Coord::new(0, 0),
            sub_world_id: (0, 0),
            world_seed: 0,
            visible_tiles: Vec::new(),
            explored: Vec::new(),
            explored_width: 0,
            entities: Vec::new(),
            items_ground: Vec::new(),
            message_log: Vec::new(),
            hp: 100,
            max_hp: 100,
            stamina: 100,
            thirst: 100,
            hunger: 100,
            seq: 0,
        }
    }
}

impl GameState {
    pub fn apply_snapshot(&mut self, msg: &ServerMessage) {
        if let ServerMessage::WorldState {
            seq,
            player_pos,
            visible_tiles,
            explored,
            explored_width,
            entities,
            items_ground,
            message_log,
            hp,
            stamina,
            thirst,
            hunger,
        } = msg
        {
            self.seq = *seq;
            self.player_pos = *player_pos;
            self.visible_tiles = visible_tiles.clone();
            self.explored = explored.clone();
            self.explored_width = *explored_width;
            self.entities = entities.clone();
            self.items_ground = items_ground.clone();
            self.message_log = message_log.clone();
            self.hp = *hp;
            self.stamina = *stamina;
            self.thirst = *thirst;
            self.hunger = *hunger;
        }
    }

    pub fn apply_snapshot_owned(&mut self, msg: ServerMessage) {
        if let ServerMessage::WorldState {
            seq,
            player_pos,
            visible_tiles,
            explored,
            explored_width,
            entities,
            items_ground,
            message_log,
            hp,
            stamina,
            thirst,
            hunger,
        } = msg
        {
            self.seq = seq;
            self.player_pos = player_pos;
            self.visible_tiles = visible_tiles;
            self.explored = explored;
            self.explored_width = explored_width;
            self.entities = entities;
            self.items_ground = items_ground;
            self.message_log = message_log;
            self.hp = hp;
            self.stamina = stamina;
            self.thirst = thirst;
            self.hunger = hunger;
        }
    }

    pub fn apply_delta(&mut self, msg: &ServerMessage) {
        match msg {
            ServerMessage::EntityMoved { id, from: _, to } => {
                if let Some(entity) = self.entities.iter_mut().find(|e| e.id == *id) {
                    entity.pos = *to;
                }
                if *id == self.player_id {
                    self.player_pos = *to;
                }
            }
            ServerMessage::EntityJoined { entity } => {
                self.entities.retain(|e| e.id != entity.id);
                self.entities.push(entity.clone());
            }
            ServerMessage::EntityLeft { id } => {
                self.entities.retain(|e| e.id != *id);
            }
            ServerMessage::AttackResult {
                attacker: _,
                target,
                damage,
                killed: _,
            } => {
                if let Some(entity) = self.entities.iter_mut().find(|e| e.id == *target) {
                    entity.hp = (entity.hp - damage).max(0);
                }
                if *target == self.player_id {
                    self.hp = (self.hp - damage).max(0);
                }
            }
            ServerMessage::ChatMessage {
                player_name,
                text,
                ..
            } => {
                self.message_log.push(LogEntry {
                    text: format!("{}: {}", player_name, text),
                    color: 0xFFFFFF,
                    turn: 0,
                });
            }
            ServerMessage::LoginAccepted {
                player_id,
                sub_world_id,
                world_seed,
            } => {
                self.player_id = *player_id;
                self.sub_world_id = *sub_world_id;
                self.world_seed = *world_seed;
            }
            ServerMessage::SubWorldTransfer {
                new_sub_world_id,
                pos,
            } => {
                self.sub_world_id = *new_sub_world_id;
                self.player_pos = *pos;
            }
            _ => {}
        }
    }

    pub fn get_visible_tiles(&self) -> &[TileData] {
        &self.visible_tiles
    }

    pub fn get_entities(&self) -> &[EntityData] {
        &self.entities
    }

    pub fn get_items_ground(&self) -> &[(Coord, ItemStack)] {
        &self.items_ground
    }

    pub fn get_message_log(&self) -> &[LogEntry] {
        &self.message_log
    }
}
