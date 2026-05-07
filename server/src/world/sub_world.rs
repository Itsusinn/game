use std::collections::HashMap;

use tokio::sync::mpsc;

use crate::world::entity::{EntityManager, EntityType};
use crate::world::map::GameMap;
use crate::world::tile::TileType;
use crate::world::worldgen;
use protocol::*;

pub struct SubWorld {
    pub id: (i64, i64),
    pub map: GameMap,
    pub entities: EntityManager,
    pub players: HashMap<u32, PlayerState>,
    pub turn: u64,
    pub message_log: Vec<LogEntry>,
}

pub struct PlayerState {
    pub pos: Coord,
    pub tx: mpsc::Sender<ServerMessage>,
    pub hp: i32,
    pub max_hp: i32,
    pub stamina: i32,
    pub hunger: i32,
    pub thirst: i32,
}

pub enum SubWorldCmd {
    PlayerJoin {
        player_id: u32,
        pos: Coord,
        tx: mpsc::Sender<ServerMessage>,
    },
    PlayerLeave {
        player_id: u32,
    },
    PlayerAction {
        player_id: u32,
        action: ActionType,
        target: Option<Coord>,
    },
}

impl SubWorld {
    pub fn new(id: (i64, i64), world_seed: u64) -> Self {
        let mut map = GameMap::new();
        let mut entities = EntityManager::new();
        worldgen::generate_sub_world(&mut map, &mut entities, world_seed, id);

        Self {
            id,
            map,
            entities,
            players: HashMap::new(),
            turn: 0,
            message_log: Vec::new(),
        }
    }

    pub async fn run(&mut self, mut rx: mpsc::Receiver<SubWorldCmd>) {
        let mut ai_interval = tokio::time::interval(std::time::Duration::from_millis(200));

        loop {
            tokio::select! {
                cmd = rx.recv() => {
                    match cmd {
                        Some(cmd) => self.handle_cmd(cmd).await,
                        None => break,
                    }
                }
                _ = ai_interval.tick() => {
                    if !self.players.is_empty() {
                        self.turn += 1;
                        self.advance_ai();
                        self.broadcast_all().await;
                    }
                }
            }
        }
    }

    async fn handle_cmd(&mut self, cmd: SubWorldCmd) {
        match cmd {
            SubWorldCmd::PlayerJoin { player_id, pos, tx } => {
                self.message_log.push(LogEntry {
                    text: format!("Player {} joined sub-world {:?}", player_id, self.id),
                    color: 0x00FF00,
                    turn: self.turn,
                });

                self.entities.spawn(
                    format!("Player_{}", player_id),
                    EntityType::Player,
                    pos,
                    100,
                );

                self.players.insert(
                    player_id,
                    PlayerState {
                        pos,
                        tx,
                        hp: 100,
                        max_hp: 100,
                        stamina: 100,
                        hunger: 100,
                        thirst: 100,
                    },
                );

                // Send initial world state to the new player
                let state = self.build_world_state_for(player_id);
                if let Some(ps) = self.players.get(&player_id) {
                    let _ = ps.tx.send(state).await;
                }
            }

            SubWorldCmd::PlayerLeave { player_id } => {
                self.players.remove(&player_id);
                self.entities.remove(player_id);
                self.message_log.push(LogEntry {
                    text: format!("Player {} left sub-world {:?}", player_id, self.id),
                    color: 0xFF4444,
                    turn: self.turn,
                });
            }

            SubWorldCmd::PlayerAction {
                player_id,
                action,
                target: _,
            } => {
                if let Some(ps) = self.players.get(&player_id) {
                    let pos = ps.pos;
                    let new_pos = match action {
                        ActionType::MoveUp => Coord::new(pos.x, pos.y - 1),
                        ActionType::MoveDown => Coord::new(pos.x, pos.y + 1),
                        ActionType::MoveLeft => Coord::new(pos.x - 1, pos.y),
                        ActionType::MoveRight => Coord::new(pos.x + 1, pos.y),
                        ActionType::MoveUpLeft => Coord::new(pos.x - 1, pos.y - 1),
                        ActionType::MoveUpRight => Coord::new(pos.x + 1, pos.y - 1),
                        ActionType::MoveDownLeft => Coord::new(pos.x - 1, pos.y + 1),
                        ActionType::MoveDownRight => Coord::new(pos.x + 1, pos.y + 1),
                        ActionType::Wait => pos,
                        _ => {
                            self.message_log.push(LogEntry {
                                text: format!(
                                    "Player {} action {:?} not implemented",
                                    player_id, action
                                ),
                                color: 0xFFFF00,
                                turn: self.turn,
                            });
                            return;
                        }
                    };

                    if new_pos != pos {
                        if self.map.is_passable(new_pos.x, new_pos.y)
                            && self
                                .entities
                                .entities_in_radius(new_pos.x, new_pos.y, 0)
                                .is_empty()
                        {
                            self.entities.move_entity(player_id, new_pos);
                            if let Some(ps) = self.players.get_mut(&player_id) {
                                ps.pos = new_pos;

                                // Check for stairs
                                if let Some(tile) = self.map.get_tile(new_pos.x, new_pos.y) {
                                    match tile.tile_type {
                                        TileType::StairsDown => {
                                            self.message_log.push(LogEntry {
                                                text: format!(
                                                    "Player {} found stairs down",
                                                    player_id
                                                ),
                                                color: 0xFFFF00,
                                                turn: self.turn,
                                            });
                                        }
                                        TileType::StairsUp => {
                                            self.message_log.push(LogEntry {
                                                text: format!(
                                                    "Player {} found stairs up",
                                                    player_id
                                                ),
                                                color: 0xFFFF00,
                                                turn: self.turn,
                                            });
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn advance_ai(&mut self) {
        // Move enemies toward nearest player
        let player_positions: Vec<(u32, Coord)> =
            self.players.iter().map(|(id, ps)| (*id, ps.pos)).collect();

        if player_positions.is_empty() {
            return;
        }

        let entity_ids: Vec<u32> = self.entities.all().map(|e| e.id).collect();
        for eid in entity_ids {
            let entity = match self.entities.get(eid) {
                Some(e) => e,
                None => continue,
            };

            if matches!(entity.entity_type, EntityType::Player) {
                continue;
            }

            let (_pid, target_pos) = player_positions[0];
            let dx = (target_pos.x - entity.pos.x).signum();
            let dy = (target_pos.y - entity.pos.y).signum();

            let new_pos = Coord::new(entity.pos.x + dx, entity.pos.y + dy);
            if self.map.is_passable(new_pos.x, new_pos.y)
                && self
                    .entities
                    .entities_in_radius(new_pos.x, new_pos.y, 0)
                    .is_empty()
            {
                self.entities.move_entity(eid, new_pos);
            }
        }
    }

    async fn broadcast_all(&mut self) {
        let player_ids: Vec<u32> = self.players.keys().copied().collect();

        for pid in player_ids {
            let state = self.build_world_state_for(pid);
            if let Some(ps) = self.players.get(&pid) {
                if ps.tx.send(state).await.is_err() {
                    self.players.remove(&pid);
                }
            }
        }
    }

    fn build_world_state_for(&mut self, player_id: u32) -> ServerMessage {
        let ps = match self.players.get(&player_id) {
            Some(ps) => ps,
            None => {
                return ServerMessage::Error {
                    code: 1,
                    text: "player not found".into(),
                }
            }
        };

        let view_radius = 15;
        let (cx, cy) = (ps.pos.x, ps.pos.y);

        // Mark tiles as explored
        for dy in -view_radius..=view_radius {
            for dx in -view_radius..=view_radius {
                self.map.set_explored(cx + dx, cy + dy, true);
            }
        }

        let visible_tiles: Vec<TileData> = self
            .map
            .tiles_in_radius(cx, cy, view_radius)
            .iter()
            .map(|(x, y, tile)| {
                let global = local_to_global(Coord::new(*x, *y), self.id);
                TileData {
                    pos: global,
                    tile_type: match tile.tile_type {
                        TileType::Floor => 0,
                        TileType::Wall => 1,
                        TileType::Door { open } => if open { 2 } else { 3 },
                        TileType::Water => 4,
                        TileType::StairsUp => 5,
                        TileType::StairsDown => 6,
                    },
                    flags: tile.flags,
                    fg_color: tile.fg_color(),
                    bg_color: tile.bg_color(),
                }
            })
            .collect();

        let entities: Vec<EntityData> = self
            .entities
            .entities_in_radius(cx, cy, view_radius)
            .iter()
            .map(|e| EntityData {
                id: e.id,
                entity_type: e.entity_type.to_u8(),
                name: e.name.clone(),
                pos: local_to_global(e.pos, self.id),
                hp: e.hp,
                max_hp: e.max_hp,
                is_player: matches!(e.entity_type, EntityType::Player),
            })
            .collect();

        // Build explored array
        let mut explored: Vec<bool> = Vec::new();
        let mut explored_width: u32 = 0;
        for dy in -view_radius..=view_radius {
            for dx in -view_radius..=view_radius {
                let x = cx + dx;
                let y = cy + dy;
                if self.map.in_bounds(x, y) {
                    explored.push(self.map.explored[self.map.index(x, y)]);
                } else {
                    explored.push(false);
                }
            }
            if explored_width == 0 {
                explored_width = (view_radius * 2 + 1) as u32;
            }
        }

        ServerMessage::WorldState {
            seq: self.turn as u32,
            player_pos: local_to_global(ps.pos, self.id),
            visible_tiles,
            explored,
            explored_width,
            entities,
            items_ground: Vec::new(),
            message_log: self
                .message_log
                .iter()
                .rev()
                .take(20)
                .cloned()
                .collect(),
            hp: ps.hp,
            stamina: ps.stamina,
            hunger: ps.hunger,
            thirst: ps.thirst,
        }
    }
}

pub fn local_to_global(local: Coord, sw_id: (i64, i64)) -> Coord {
    Coord::new(
        local.x + (sw_id.0 * SUBWORLD_SIZE as i64) as i32,
        local.y + (sw_id.1 * SUBWORLD_SIZE as i64) as i32,
    )
}

pub fn global_to_local(global: Coord) -> ((i64, i64), Coord) {
    let sw_id = global.sub_world_id();
    let local = Coord::new(
        global.x - (sw_id.0 * SUBWORLD_SIZE as i64) as i32,
        global.y - (sw_id.1 * SUBWORLD_SIZE as i64) as i32,
    );
    (sw_id, local)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coord_conversion() {
        let global = Coord::new(600, -100);
        let (sw_id, local) = global_to_local(global);
        let roundtrip = local_to_global(local, sw_id);
        assert_eq!(global, roundtrip);
        assert_eq!(sw_id, (1, -1));
    }

    #[tokio::test]
    async fn test_sub_world_creation() {
        let sw = SubWorld::new((0, 0), 12345);
        assert_eq!(sw.id, (0, 0));
        assert!(!sw.map.is_passable(0, 0)); // border is wall

        // Verify some floor tiles exist
        let mut floor_count = 0;
        for y in 0..SUBWORLD_SIZE {
            for x in 0..SUBWORLD_SIZE {
                if sw.map.is_passable(x, y) {
                    floor_count += 1;
                }
            }
        }
        assert!(floor_count > 100, "expected >100 floor tiles, got {floor_count}");

        // Verify enemies were spawned
        assert!(sw.entities.len() > 0, "expected some entities");
    }
}
