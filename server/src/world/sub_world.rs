use std::collections::{HashMap, HashSet};

use tokio::sync::{mpsc, oneshot};
use tracing::{debug, info, instrument, warn};

use crate::storage::save::SavedMap;
use crate::world::ai;
use crate::world::combat;
use crate::world::entity::{EntityManager, EntityType};
use crate::world::fov;
use crate::world::item::ItemManager;
use crate::world::map::GameMap;
use crate::world::messagelog::MessageLog;
use crate::world::tile::TileType;
use crate::world::worldgen;
use protocol::*;

pub struct SubWorld {
    pub id: (i64, i64),
    pub map: GameMap,
    pub entities: EntityManager,
    pub items: ItemManager,
    pub players: HashMap<u32, PlayerState>,
    pub turn: u64,
    pub log: MessageLog,
    pub event_tx: Option<mpsc::UnboundedSender<SubWorldEvent>>,
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
    Snapshot {
        reply: oneshot::Sender<SavedMap>,
    },
}

pub enum SubWorldEvent {
    TransferPlayer {
        player_id: u32,
        from_sw: (i64, i64),
        to_sw: (i64, i64),
        pos: Coord,
        tx: mpsc::Sender<ServerMessage>,
    },
}

impl SubWorld {
    #[instrument(level = "info", fields(sw_id = ?id))]
    pub fn new(id: (i64, i64), world_seed: u64) -> Self {
        info!("Generating new sub-world");
        let mut map = GameMap::new();
        let mut entities = EntityManager::new();
        let mut items = ItemManager::new();
        worldgen::generate_sub_world(&mut map, &mut entities, &mut items, world_seed, id);

        info!(rooms = ?entities.all().count(), "Sub-world generated");
        Self {
            id,
            map,
            entities,
            items,
            players: HashMap::new(),
            turn: 0,
            log: MessageLog::default(),
            event_tx: None,
        }
    }

    #[instrument(level = "info", skip(self, rx))]
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
                info!(?player_id, ?pos, sw = ?self.id, "Player joined sub-world");

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

                let state = self.build_world_state_for(player_id);
                if let Some(ps) = self.players.get(&player_id) {
                    let _ = ps.tx.send(state).await;
                }
            }

            SubWorldCmd::PlayerLeave { player_id } => {
                info!(?player_id, sw = ?self.id, "Player left sub-world");
                self.players.remove(&player_id);
                self.entities.remove(player_id);
                self.log.info(
                    self.turn,
                    format!("Player {} left sub-world {:?}", player_id, self.id),
                );
            }

            SubWorldCmd::PlayerAction {
                player_id,
                action,
                target: _,
            } => {
                debug!(?player_id, ?action, "Player action received");
                if let Some(ps) = self.players.get(&player_id) {
                    let pos = ps.pos;
                    let new_pos = match &action {
                        ActionType::MoveUp => Coord::new(pos.x, pos.y - 1),
                        ActionType::MoveDown => Coord::new(pos.x, pos.y + 1),
                        ActionType::MoveLeft => Coord::new(pos.x - 1, pos.y),
                        ActionType::MoveRight => Coord::new(pos.x + 1, pos.y),
                        ActionType::MoveUpLeft => Coord::new(pos.x - 1, pos.y - 1),
                        ActionType::MoveUpRight => Coord::new(pos.x + 1, pos.y - 1),
                        ActionType::MoveDownLeft => Coord::new(pos.x - 1, pos.y + 1),
                        ActionType::MoveDownRight => Coord::new(pos.x + 1, pos.y + 1),
                        ActionType::Wait => pos,
                        ActionType::MeleeAttack => {
                            debug!("Player melee attack");
                            self.handle_player_attack(player_id);
                            return;
                        }
                        ActionType::Pickup => {
                            debug!("Player pickup");
                            self.handle_pickup(player_id);
                            return;
                        }
                        _ => {
                            warn!(?action, "Action not implemented");
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
                            debug!(from = ?pos, to = ?new_pos, "Player moved");
                            self.entities.move_entity(player_id, new_pos);
                            if let Some(ps) = self.players.get_mut(&player_id) {
                                ps.pos = new_pos;

                                if let Some(tile) = self.map.get_tile(new_pos.x, new_pos.y) {
                                    match tile.tile_type {
                                        TileType::StairsDown => {
                                            self.emit_transfer(
                                                player_id,
                                                (self.id.0, self.id.1 + 1),
                                                Coord::new(256, 256),
                                            );
                                            return;
                                        }
                                        TileType::StairsUp => {
                                            self.emit_transfer(
                                                player_id,
                                                (self.id.0, self.id.1 - 1),
                                                Coord::new(256, 256),
                                            );
                                            return;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
            SubWorldCmd::Snapshot { reply } => {
                let saved = SavedMap::from(&self.map);
                let _ = reply.send(saved);
            }
        }
    }

    #[instrument(level = "debug", skip(self))]
    fn handle_player_attack(&mut self, player_id: u32) {
        let player_pos = match self.players.get(&player_id) {
            Some(ps) => ps.pos,
            None => return,
        };

        // Find adjacent enemy
        let target = self
            .entities
            .all()
            .filter(|e| !matches!(e.entity_type, EntityType::Player))
            .find(|e| {
                let dx = (e.pos.x - player_pos.x).abs();
                let dy = (e.pos.y - player_pos.y).abs();
                dx <= 1 && dy <= 1
            });

        let target_id = match target {
            Some(e) => e.id,
            None => {
                self.log.info(self.turn, "No enemy nearby to attack.");
                return;
            }
        };

        let (atk, _, base_dmg) = combat::get_entity_stats(0);
        let (_, def, _) = combat::get_entity_stats(
            self.entities
                .get(target_id)
                .map(|e| e.entity_type.to_u8())
                .unwrap_or(1),
        );

        let result = combat::melee_attack(atk, def, base_dmg);
        let attacker_name = format!("Player_{}", player_id);
        let defender_name = self
            .entities
            .get(target_id)
            .map(|e| e.name.clone())
            .unwrap_or_else(|| "unknown".into());

        info!(target_id, damage = result.damage, critical = result.critical, "Melee attack result");

        self.log.combat(
            self.turn,
            &attacker_name,
            &defender_name,
            result.damage,
            result.critical,
        );

        if result.damage > 0 {
            if let Some(target) = self.entities.get_mut(target_id) {
                target.hp -= result.damage;
                if target.hp <= 0 {
                    self.log.death(self.turn, &defender_name);
                    self.entities.remove(target_id);
                }
            }
        }
    }

    fn handle_pickup(&mut self, player_id: u32) {
        let player_pos = match self.players.get(&player_id) {
            Some(ps) => ps.pos,
            None => return,
        };

        let ground: Vec<u32> = self
            .items
            .items_at(player_pos.x, player_pos.y)
            .iter()
            .map(|gi| gi.stack.item.id)
            .collect();

        if ground.is_empty() {
            debug!("Nothing to pick up");
            self.log.info(self.turn, "Nothing to pick up here.");
            return;
        }

        let mut picked_up = 0;
        for item_id in ground {
            if self
                .items
                .remove_at(player_pos.x, player_pos.y, item_id)
                .is_some()
            {
                picked_up += 1;
            }
        }
        debug!(picked_up, "Items picked up");
    }

    fn advance_ai(&mut self) {
        let entity_count = self.entities.all().count();
        debug!(entity_count, turn = self.turn, "Advancing AI");
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

            let target = player_positions[0].1;
            let (_, def, _) = combat::get_entity_stats(entity.entity_type.to_u8());

            // Check if adjacent to player for attack
            let dist = (entity.pos.x - target.x).abs().max((entity.pos.y - target.y).abs());
            if dist <= 1 {
                let (atk, _, base_dmg) = combat::get_entity_stats(entity.entity_type.to_u8());
                let result = combat::melee_attack(atk, def, base_dmg);

                let player_name = format!("Player_{}", player_positions[0].0);
                self.log.combat(
                    self.turn,
                    &entity.name,
                    &player_name,
                    result.damage,
                    result.critical,
                );

                if result.damage > 0 {
                    if let Some(ps) = self.players.get_mut(&player_positions[0].0) {
                        ps.hp -= result.damage;
                        if ps.hp <= 0 {
                            ps.hp = 0;
                            self.log.death(self.turn, &player_name);
                        }
                    }
                }
                continue;
            }

            // Use AI to move toward player
            if let Some(action) = ai::ai_action(entity, &self.map, &self.entities, target, 8, 15) {
                if let Some(new_pos) = ai_action_to_pos(entity.pos, &action) {
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
        }
    }

    #[instrument(level = "debug", skip(self))]
    fn emit_transfer(&mut self, player_id: u32, to_sw: (i64, i64), pos: Coord) {
        if let Some(event_tx) = &self.event_tx {
            let player_tx = match self.players.get(&player_id) {
                Some(ps) => ps.tx.clone(),
                None => return,
            };

            // Send transfer notification to client
            let transfer_msg = ServerMessage::SubWorldTransfer {
                new_sub_world_id: to_sw,
                pos: local_to_global(pos, to_sw),
            };
            let _ = player_tx.try_send(transfer_msg);

            // Emit event to WorldManager for server-side transfer
            let _ = event_tx.send(SubWorldEvent::TransferPlayer {
                player_id,
                from_sw: self.id,
                to_sw,
                pos,
                tx: player_tx,
            });

            // Remove from this sub-world
            self.players.remove(&player_id);
            self.entities.remove(player_id);

            self.log.info(
                self.turn,
                format!("Player {} transferred to {:?}", player_id, to_sw),
            );
        }
    }

    #[instrument(level = "trace", skip(self))]
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

        let view_radius: i32 = 15;
        let (cx, cy) = (ps.pos.x, ps.pos.y);

        // Compute FOV
        let origin = Coord::new(cx, cy);
        let mut visible = HashSet::new();
        fov::compute_fov(&self.map, origin, view_radius, &mut visible);

        // Mark visible tiles as explored
        for coord in &visible {
            self.map.set_explored(coord.x, coord.y, true);
        }

        // Also mark tiles around visible as explored (explored but not currently visible)
        let visible_tiles: Vec<TileData> = visible
            .iter()
            .filter_map(|coord| {
                self.map.get_tile(coord.x, coord.y).map(|tile| {
                    let global = local_to_global(*coord, self.id);
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
            })
            .collect();

        // Only show entities in visible tiles
        let entities: Vec<EntityData> = self
            .entities
            .all()
            .filter(|e| visible.contains(&e.pos))
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

        // Ground items in visible area
        let items_ground: Vec<(Coord, protocol::ItemStack)> = self
            .items
            .all_ground_items()
            .into_iter()
            .filter(|(coord, _)| visible.contains(coord))
            .collect();

        // Build explored array for view area
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
            items_ground,
            message_log: self.log.recent(20),
            hp: ps.hp,
            stamina: ps.stamina,
            hunger: ps.hunger,
            thirst: ps.thirst,
        }
    }
}

fn ai_action_to_pos(pos: Coord, action: &ActionType) -> Option<Coord> {
    match action {
        ActionType::MoveUp => Some(Coord::new(pos.x, pos.y - 1)),
        ActionType::MoveDown => Some(Coord::new(pos.x, pos.y + 1)),
        ActionType::MoveLeft => Some(Coord::new(pos.x - 1, pos.y)),
        ActionType::MoveRight => Some(Coord::new(pos.x + 1, pos.y)),
        ActionType::MoveUpLeft => Some(Coord::new(pos.x - 1, pos.y - 1)),
        ActionType::MoveUpRight => Some(Coord::new(pos.x + 1, pos.y - 1)),
        ActionType::MoveDownLeft => Some(Coord::new(pos.x - 1, pos.y + 1)),
        ActionType::MoveDownRight => Some(Coord::new(pos.x + 1, pos.y + 1)),
        ActionType::Wait => Some(pos),
        _ => None,
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
        assert!(!sw.map.is_passable(0, 0));

        let mut floor_count = 0;
        for y in 0..SUBWORLD_SIZE {
            for x in 0..SUBWORLD_SIZE {
                if sw.map.is_passable(x, y) {
                    floor_count += 1;
                }
            }
        }
        assert!(floor_count > 100, "expected >100 floor tiles, got {floor_count}");
        assert!(sw.entities.len() > 0, "expected some entities");
    }
}
