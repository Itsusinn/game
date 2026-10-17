use std::collections::HashMap;

use tokio::sync::{mpsc, oneshot};
use tracing::{info, instrument, warn};

use crate::storage::save::{SavedMap, SavedWorld};
use crate::world::sub_world::{global_to_local, SubWorldCmd, SubWorldEvent};
use protocol::*;

pub struct WorldManager {
    saved_maps: HashMap<(i64, i64), SavedMap>,
    sub_worlds: HashMap<(i64, i64), mpsc::Sender<SubWorldCmd>>,
    player_sw: HashMap<u32, (i64, i64)>,
    next_player_id: u32,
    pub world_seed: u64,
    event_tx: mpsc::UnboundedSender<SubWorldEvent>,
    event_rx: Option<mpsc::UnboundedReceiver<SubWorldEvent>>,
}

impl WorldManager {
    #[instrument(level = "info")]
    pub fn new(world_seed: u64) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        Self {
            saved_maps: HashMap::new(),
            sub_worlds: HashMap::new(),
            player_sw: HashMap::new(),
            next_player_id: 1,
            world_seed,
            event_tx,
            event_rx: Some(event_rx),
        }
    }

    pub fn from_saved(saved: SavedWorld) -> anyhow::Result<Self> {
        for map in saved.sub_worlds.values() {
            map.restore()?;
        }
        let mut manager = Self::new(saved.world_seed);
        manager.saved_maps = saved.sub_worlds;
        Ok(manager)
    }

    /// Take the event receiver. May only be called once; subsequent calls return None.
    pub fn take_event_rx(&mut self) -> Option<mpsc::UnboundedReceiver<SubWorldEvent>> {
        self.event_rx.take()
    }

    #[instrument(level = "info", skip(self))]
    pub async fn get_or_create_sub_world(
        &mut self,
        sw_id: (i64, i64),
    ) -> mpsc::Sender<SubWorldCmd> {
        if let Some(tx) = self.sub_worlds.get(&sw_id) {
            return tx.clone();
        }

        let (cmd_tx, cmd_rx) = mpsc::channel(256);

        let mut sw = crate::world::sub_world::SubWorld::new(sw_id, self.world_seed);
        if let Some(saved) = self.saved_maps.remove(&sw_id) {
            sw.map = saved
                .restore()
                .expect("loaded maps validated before manager construction");
            let blocked: Vec<_> = sw
                .entities
                .all()
                .filter(|e| !sw.map.is_passable(e.pos.x, e.pos.y))
                .map(|e| e.id)
                .collect();
            for id in blocked {
                sw.entities.remove(id);
            }
        }
        sw.event_tx = Some(self.event_tx.clone());

        tokio::spawn(async move {
            sw.run(cmd_rx).await;
        });

        self.sub_worlds.insert(sw_id, cmd_tx.clone());
        cmd_tx
    }

    pub fn allocate_player_id(&mut self) -> u32 {
        let id = self.next_player_id;
        self.next_player_id += 1;
        id
    }

    #[instrument(level = "info", skip(self, tx))]
    pub async fn register_player(
        &mut self,
        player_id: u32,
        global_pos: Coord,
        tx: mpsc::Sender<ServerMessage>,
    ) {
        let (sw_id, local_pos) = global_to_local(global_pos);
        let sw_tx = self.get_or_create_sub_world(sw_id).await;
        self.player_sw.insert(player_id, sw_id);

        let _ = sw_tx
            .send(SubWorldCmd::PlayerJoin {
                previous: None,
                player_id,
                pos: local_pos,
                tx,
            })
            .await;
    }

    #[instrument(level = "debug", skip(self))]
    pub async fn unregister_player(&mut self, player_id: u32) {
        if let Some(sw_id) = self.player_sw.remove(&player_id) {
            if let Some(sw_tx) = self.sub_worlds.get(&sw_id) {
                let _ = sw_tx.send(SubWorldCmd::PlayerLeave { player_id }).await;
            }
        }
    }

    #[instrument(level = "debug", skip(self))]
    pub async fn handle_player_action(
        &mut self,
        player_id: u32,
        seq: u32,
        action: ActionType,
        target: Option<Coord>,
    ) {
        let sw_id = match self.player_sw.get(&player_id) {
            Some(id) => *id,
            None => return,
        };

        let target_local = target.map(|t| {
            let (_, local) = global_to_local(t);
            local
        });

        if let Some(sw_tx) = self.sub_worlds.get(&sw_id) {
            let _ = sw_tx
                .send(SubWorldCmd::PlayerAction {
                    player_id,
                    seq,
                    action,
                    target: target_local,
                })
                .await;
        }
    }

    /// Move a player from their current sub-world to `to_sw_id` at `local_pos`,
    /// using the provided per-player message channel for the new sub-world.
    /// The source sub-world is expected to have already removed the player locally
    /// (see `SubWorld::emit_transfer`).
    #[instrument(level = "info", skip(self, tx))]
    pub async fn transfer_player(
        &mut self,
        player_id: u32,
        to_sw_id: (i64, i64),
        local_pos: Coord,
        tx: mpsc::Sender<ServerMessage>,
        previous: Option<crate::world::sub_world::PlayerState>,
    ) {
        let new_sw_tx = self.get_or_create_sub_world(to_sw_id).await;
        self.player_sw.insert(player_id, to_sw_id);

        if let Err(e) = new_sw_tx
            .send(SubWorldCmd::PlayerJoin {
                previous,
                player_id,
                pos: local_pos,
                tx,
            })
            .await
        {
            warn!(error = %e, "Failed to deliver PlayerJoin during transfer");
        } else {
            info!(player_id, ?to_sw_id, ?local_pos, "Player transferred");
        }
    }

    /// Snapshot every active sub-world's map and bundle them with the world seed.
    /// Used for save-on-shutdown.
    #[instrument(level = "info", skip(self))]
    pub async fn snapshot_all(&self) -> SavedWorld {
        let mut sub_worlds = self.saved_maps.clone();

        for (sw_id, sw_tx) in &self.sub_worlds {
            let (reply_tx, reply_rx) = oneshot::channel();
            if sw_tx
                .send(SubWorldCmd::Snapshot { reply: reply_tx })
                .await
                .is_err()
            {
                warn!(?sw_id, "Sub-world channel closed during snapshot");
                continue;
            }
            match reply_rx.await {
                Ok(saved) => {
                    sub_worlds.insert(*sw_id, saved);
                }
                Err(_) => {
                    warn!(?sw_id, "Sub-world dropped reply during snapshot");
                }
            }
        }

        SavedWorld {
            world_seed: self.world_seed,
            sub_worlds,
        }
    }

    pub fn player_sub_world(&self, player_id: u32) -> Option<(i64, i64)> {
        self.player_sw.get(&player_id).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::ServerMessage;

    #[tokio::test]
    async fn restored_maps_survive_loading_and_unvisited_regions_survive_resaving() {
        let mut map = crate::world::map::GameMap::new();
        map.set_tile(
            10,
            10,
            crate::world::tile::Tile::new(crate::world::tile::TileType::Water),
        );
        map.set_explored(10, 10, true);
        let maps = HashMap::from([
            ((0, 0), SavedMap::from(&map)),
            ((7, -3), SavedMap::from(&map)),
        ]);
        let mut manager = WorldManager::from_saved(SavedWorld {
            world_seed: 42,
            sub_worlds: maps,
        })
        .unwrap();
        manager.get_or_create_sub_world((0, 0)).await;
        let saved = manager.snapshot_all().await;
        assert_eq!(saved.sub_worlds.len(), 2);
        for map in saved.sub_worlds.values() {
            let index = 10 * protocol::SUBWORLD_SIZE as usize + 10;
            assert_eq!(map.tiles[index].tile_type, 4);
            assert!(map.explored[index]);
        }
    }

    #[tokio::test]
    async fn transfer_routes_player_to_new_sub_world() {
        let mut mgr = WorldManager::new(42);
        let (tx, mut rx) = mpsc::channel::<ServerMessage>(16);

        // Register at origin sub-world (0,0)
        mgr.register_player(1, Coord::new(10, 10), tx.clone()).await;
        assert_eq!(mgr.player_sub_world(1), Some((0, 0)));

        // Drain any initial WorldState the sub-world emits on join.
        while rx.try_recv().is_ok() {}

        // Transfer to (1, 0) at local pos (256, 256)
        mgr.transfer_player(1, (1, 0), Coord::new(256, 256), tx.clone(), None)
            .await;

        assert_eq!(mgr.player_sub_world(1), Some((1, 0)));
    }
}
