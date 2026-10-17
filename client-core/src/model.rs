use crate::{
    input_queue::{InputQueue, PredictedState},
    prediction::predict_move,
    state::GameState,
};
use protocol::*;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct ClientModel {
    pub state: GameState,
    pub pending: InputQueue,
    pub known_tiles: HashMap<Coord, TileData>,
    pub visible: HashSet<Coord>,
    pub logged_in: bool,
}
impl ClientModel {
    pub fn apply(&mut self, message: ServerMessage) {
        match &message {
            ServerMessage::LoginAccepted { .. } => {
                *self = Self::default();
                self.logged_in = true;
            }
            ServerMessage::SubWorldTransfer { .. } => {
                self.known_tiles.clear();
                self.visible.clear();
            }
            ServerMessage::WorldState {
                seq,
                player_pos,
                visible_tiles,
                ..
            } => {
                let region = player_pos.sub_world_id();
                if region != self.state.sub_world_id {
                    self.known_tiles.clear();
                    self.state.sub_world_id = region;
                }
                self.pending.ack_up_to(*seq);
                self.visible.clear();
                for tile in visible_tiles {
                    self.visible.insert(tile.pos);
                    self.known_tiles.insert(tile.pos, tile.clone());
                }
            }
            _ => {}
        }
        if matches!(message, ServerMessage::WorldState { .. }) {
            self.state.apply_snapshot_owned(message);
            for pending in self.pending.iter() {
                self.state.player_pos = self.predict(self.state.player_pos, &pending.action);
            }
        } else {
            self.state.apply_delta(&message);
        }
    }
    fn predict(&self, position: Coord, action: &ActionType) -> Coord {
        let next = predict_move(position, action);
        if self
            .known_tiles
            .get(&next)
            .is_some_and(|t| !t.flags.blocks_movement())
            && !self
                .state
                .entities
                .iter()
                .any(|e| e.id != self.state.player_id && e.pos == next)
        {
            next
        } else {
            position
        }
    }
    pub fn try_action(
        &mut self,
        action: ActionType,
        target: Option<Coord>,
        send: impl FnOnce(ClientMessage) -> Result<(), String>,
    ) -> Result<(), String> {
        if !self.logged_in {
            return Err("Not logged in".into());
        }
        if self.pending.pending_count() >= 64 {
            return Err("Waiting for server acknowledgements".into());
        }
        let seq = self.pending.next_seq();
        if seq == u32::MAX {
            return Err("Action sequence exhausted; reconnect".into());
        }
        let pos = self.predict(self.state.player_pos, &action);
        send(ClientMessage::PlayerAction {
            seq,
            action: action.clone(),
            target,
        })?;
        self.pending.push(action, Some(PredictedState { pos }));
        self.state.player_pos = pos;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn model() -> ClientModel {
        let mut m = ClientModel::default();
        m.apply(ServerMessage::LoginAccepted {
            player_id: 1,
            sub_world_id: (0, 0),
            world_seed: 7,
        });
        for x in 0..4 {
            m.known_tiles.insert(
                Coord::new(x, 0),
                TileData {
                    pos: Coord::new(x, 0),
                    tile_type: 0,
                    flags: TileFlags(0),
                    fg_color: 0,
                    bg_color: 0,
                },
            );
        }
        m
    }
    #[test]
    fn rejected_transport_does_not_mutate_prediction_or_pending_actions() {
        let mut m = model();
        assert!(m
            .try_action(ActionType::MoveRight, None, |_| Err("full".into()))
            .is_err());
        assert_eq!(m.state.player_pos, Coord::new(0, 0));
        assert_eq!(m.pending.pending_count(), 0);
    }

    #[test]
    fn snapshot_can_recover_region_change_without_a_transfer_notification() {
        let mut m = model();
        m.apply(ServerMessage::WorldState {
            seq: 0,
            player_pos: Coord::new(600, 0),
            visible_tiles: vec![],
            explored: vec![],
            explored_width: 0,
            entities: vec![],
            items_ground: vec![],
            message_log: vec![],
            hp: 100,
            max_hp: 100,
            inventory: vec![],
            stamina: 100,
            hunger: 100,
            thirst: 100,
        });
        assert_eq!(m.state.sub_world_id, (1, 0));
        assert!(m.known_tiles.is_empty());
    }
    #[test]
    fn authoritative_snapshot_replays_only_unacknowledged_actions() {
        let mut m = model();
        m.try_action(ActionType::MoveRight, None, |_| Ok(()))
            .unwrap();
        m.try_action(ActionType::MoveRight, None, |_| Ok(()))
            .unwrap();
        m.apply(ServerMessage::WorldState {
            seq: 1,
            player_pos: Coord::new(1, 0),
            visible_tiles: vec![],
            explored: vec![],
            explored_width: 0,
            entities: vec![],
            items_ground: vec![],
            message_log: vec![],
            hp: 100,
            max_hp: 100,
            inventory: vec![],
            stamina: 100,
            hunger: 100,
            thirst: 100,
        });
        assert_eq!(m.state.player_pos, Coord::new(2, 0));
        assert_eq!(m.pending.pending_count(), 1);
    }
}
