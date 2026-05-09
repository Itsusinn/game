use std::collections::HashMap;

use tokio::sync::mpsc;
use tracing::{info, instrument};

use crate::world::sub_world::{
    global_to_local, SubWorldCmd, SubWorldEvent,
};
use protocol::*;

pub struct WorldManager {
    sub_worlds: HashMap<(i64, i64), mpsc::Sender<SubWorldCmd>>,
    player_sw: HashMap<u32, (i64, i64)>,
    next_player_id: u32,
    pub world_seed: u64,
}

impl WorldManager {
    #[instrument(level = "info")]
    pub fn new(world_seed: u64) -> Self {
        Self {
            sub_worlds: HashMap::new(),
            player_sw: HashMap::new(),
            next_player_id: 1,
            world_seed,
        }
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
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        let mut sw = crate::world::sub_world::SubWorld::new(sw_id, self.world_seed);
        sw.event_tx = Some(event_tx);

        tokio::spawn(async move {
            sw.run(cmd_rx).await;
        });

        // Spawn event listener that forwards transfer events
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                match event {
                    SubWorldEvent::TransferPlayer {
                        player_id,
                        from_sw: _,
                        to_sw,
                        pos,
                        tx,
                    } => {
                        info!(
                            player_id,
                            to_sw = ?to_sw,
                            pos = ?pos,
                            "Transferring player to sub-world"
                        );
                        // The player is already removed from the old sub-world.
                        // We need to register in the new one.
                        // This would require access to WorldManager from here.
                        // For now, the event is logged.
                        let _ = (player_id, to_sw, pos, tx);
                    }
                }
            }
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
                let _ = sw_tx
                    .send(SubWorldCmd::PlayerLeave { player_id })
                    .await;
            }
        }
    }

    #[instrument(level = "debug", skip(self))]
    pub async fn handle_player_action(
        &mut self,
        player_id: u32,
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
                    action,
                    target: target_local,
                })
                .await;
        }
    }

    #[instrument(level = "debug", skip(self))]
    pub async fn transfer_player(
        &mut self,
        player_id: u32,
        to_sw_id: (i64, i64),
        local_pos: Coord,
    ) {
        if let Some(old_sw_id) = self.player_sw.remove(&player_id) {
            if let Some(sw_tx) = self.sub_worlds.get(&old_sw_id) {
                let _ = sw_tx
                    .send(SubWorldCmd::PlayerLeave { player_id })
                    .await;
            }
        }

        self.player_sw.insert(player_id, to_sw_id);
    }
}
