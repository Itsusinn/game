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

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(id: u32, x: i32, y: i32) -> EntityData {
        EntityData {
            id,
            entity_type: 1,
            name: format!("e{id}"),
            pos: Coord::new(x, y),
            hp: 10,
            max_hp: 10,
            is_player: false,
        }
    }

    fn world_state_msg() -> ServerMessage {
        ServerMessage::WorldState {
            seq: 7,
            player_pos: Coord::new(50, 60),
            visible_tiles: vec![TileData {
                pos: Coord::new(1, 2),
                tile_type: 0,
                flags: TileFlags(0),
                fg_color: 0xFFFFFF,
                bg_color: 0,
            }],
            explored: vec![true, false, true],
            explored_width: 3,
            entities: vec![entity(1, 50, 60), entity(2, 51, 60)],
            items_ground: vec![(
                Coord::new(50, 60),
                ItemStack {
                    item_id: 1,
                    quantity: 2,
                },
            )],
            message_log: vec![LogEntry {
                text: "spawn".into(),
                color: 0xAAAAAA,
                turn: 1,
            }],
            hp: 88,
            stamina: 77,
            thirst: 66,
            hunger: 55,
        }
    }

    #[test]
    fn apply_snapshot_owned_populates_all_fields() {
        let mut s = GameState::default();
        s.apply_snapshot_owned(world_state_msg());

        assert_eq!(s.seq, 7);
        assert_eq!(s.player_pos, Coord::new(50, 60));
        assert_eq!(s.visible_tiles.len(), 1);
        assert_eq!(s.explored, vec![true, false, true]);
        assert_eq!(s.explored_width, 3);
        assert_eq!(s.entities.len(), 2);
        assert_eq!(s.items_ground.len(), 1);
        assert_eq!(s.message_log.len(), 1);
        assert_eq!(s.hp, 88);
        assert_eq!(s.stamina, 77);
        assert_eq!(s.thirst, 66);
        assert_eq!(s.hunger, 55);
    }

    #[test]
    fn apply_snapshot_borrowed_matches_owned() {
        let mut a = GameState::default();
        let mut b = GameState::default();
        let msg = world_state_msg();
        a.apply_snapshot(&msg);
        b.apply_snapshot_owned(msg);
        assert_eq!(a.player_pos, b.player_pos);
        assert_eq!(a.entities.len(), b.entities.len());
        assert_eq!(a.hp, b.hp);
    }

    #[test]
    fn apply_delta_entity_moved() {
        let mut s = GameState {
            entities: vec![entity(1, 0, 0), entity(2, 5, 5)],
            ..Default::default()
        };
        s.apply_delta(&ServerMessage::EntityMoved {
            id: 2,
            from: Coord::new(5, 5),
            to: Coord::new(6, 5),
        });
        assert_eq!(s.entities[1].pos, Coord::new(6, 5));
        assert_eq!(s.entities[0].pos, Coord::new(0, 0));
    }

    #[test]
    fn apply_delta_entity_moved_updates_player_pos() {
        let mut s = GameState {
            player_id: 9,
            entities: vec![entity(9, 1, 1)],
            ..Default::default()
        };
        s.apply_delta(&ServerMessage::EntityMoved {
            id: 9,
            from: Coord::new(1, 1),
            to: Coord::new(2, 1),
        });
        assert_eq!(s.player_pos, Coord::new(2, 1));
    }

    #[test]
    fn apply_delta_entity_joined_and_left() {
        let mut s = GameState::default();
        s.apply_delta(&ServerMessage::EntityJoined {
            entity: entity(7, 3, 3),
        });
        assert_eq!(s.entities.len(), 1);
        assert_eq!(s.entities[0].id, 7);

        s.apply_delta(&ServerMessage::EntityLeft { id: 7 });
        assert!(s.entities.is_empty());
    }

    #[test]
    fn apply_delta_attack_result_subtracts_hp() {
        let mut s = GameState {
            player_id: 1,
            hp: 50,
            entities: vec![entity(2, 0, 0)],
            ..Default::default()
        };
        s.apply_delta(&ServerMessage::AttackResult {
            attacker: 1,
            target: 2,
            damage: 4,
            killed: false,
        });
        assert_eq!(s.entities[0].hp, 6);

        s.apply_delta(&ServerMessage::AttackResult {
            attacker: 2,
            target: 1,
            damage: 7,
            killed: false,
        });
        assert_eq!(s.hp, 43);
    }

    #[test]
    fn apply_delta_chat_message_appends_log() {
        let mut s = GameState::default();
        s.apply_delta(&ServerMessage::ChatMessage {
            player_id: 1,
            player_name: "alice".to_string(),
            text: "hi".to_string(),
        });
        assert_eq!(s.message_log.len(), 1);
        assert!(s.message_log[0].text.contains("alice"));
        assert!(s.message_log[0].text.contains("hi"));
    }

    #[test]
    fn apply_delta_login_accepted() {
        let mut s = GameState::default();
        s.apply_delta(&ServerMessage::LoginAccepted {
            player_id: 42,
            sub_world_id: (3, 4),
            world_seed: 1234,
        });
        assert_eq!(s.player_id, 42);
        assert_eq!(s.sub_world_id, (3, 4));
        assert_eq!(s.world_seed, 1234);
    }

    #[test]
    fn apply_delta_sub_world_transfer() {
        let mut s = GameState::default();
        s.apply_delta(&ServerMessage::SubWorldTransfer {
            new_sub_world_id: (1, -1),
            pos: Coord::new(256, 256),
        });
        assert_eq!(s.sub_world_id, (1, -1));
        assert_eq!(s.player_pos, Coord::new(256, 256));
    }
}
