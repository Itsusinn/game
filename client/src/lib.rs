pub mod godot_bridge;
pub mod input_queue;
pub mod network_client;
pub mod prediction;
pub mod state;

use anyhow::Result;
use std::net::SocketAddr;

use input_queue::InputQueue;
use network_client::NetworkClient;
use prediction::{check_rollback, predict_move};
use protocol::*;
use state::GameState;

use godot::prelude::*;

struct CddaExtension;

#[gdextension]
unsafe impl ExtensionLibrary for CddaExtension {}

pub struct GameClient {
    pub state: GameState,
    pub input_queue: InputQueue,
    network: NetworkClient,
}

impl GameClient {
    pub async fn connect(server_addr: SocketAddr, player_name: &str) -> Result<Self> {
        let mut network = NetworkClient::connect(server_addr).await?;

        let login = ClientMessage::Login {
            version: 1,
            player_name: player_name.to_string(),
        };
        network.send_message(&login).await?;

        let mut state = GameState::default();
        let input_queue = InputQueue::new();

        // Read initial state (LoginAccepted or WorldState)
        let msg = network.recv_message().await?;
        match &msg {
            ServerMessage::LoginAccepted {
                player_id,
                sub_world_id,
                world_seed,
            } => {
                state.player_id = *player_id;
                state.sub_world_id = *sub_world_id;
                state.world_seed = *world_seed;
                println!(
                    "Logged in as player {} in sub-world {:?}",
                    player_id, sub_world_id
                );

                // Next message should be WorldState
                let ws = network.recv_message().await?;
                state.apply_snapshot_owned(ws);
            }
            ServerMessage::WorldState { .. } => {
                state.apply_snapshot_owned(msg);
            }
            _ => {
                eprintln!("Unexpected response after login: {:?}", msg);
            }
        }

        Ok(Self {
            state,
            input_queue,
            network,
        })
    }

    /// Process one frame: poll for server messages and handle them.
    /// Returns true if still connected.
    pub async fn tick(&mut self) -> Result<bool> {
        match self.network.recv_message().await {
            Ok(msg) => {
                self.handle_server_message(msg);
                Ok(true)
            }
            Err(e) => {
                eprintln!("Network error: {e}");
                Ok(false)
            }
        }
    }

    fn handle_server_message(&mut self, msg: ServerMessage) {
        match &msg {
            ServerMessage::WorldState {
                seq, player_pos, ..
            } => {
                let acked = self.input_queue.ack_up_to(*seq);
                for pending in &acked {
                    if let Some(ref predicted) = pending.predicted {
                        if let Some(corrected) = check_rollback(predicted.pos, *player_pos) {
                            println!(
                                "Rollback! seq={}: predicted ({},{}), server says ({},{})",
                                pending.seq,
                                predicted.pos.x,
                                predicted.pos.y,
                                corrected.x,
                                corrected.y
                            );
                        }
                    }
                }

                self.state.apply_snapshot_owned(msg);
            }
            _ => {
                self.state.apply_delta(&msg);
            }
        }
    }

    /// Send a move action. Predicts the new position locally and returns the predicted position.
    pub async fn send_move(&mut self, action: ActionType) -> Result<Coord> {
        let predicted_pos = predict_move(self.state.player_pos, &action);

        self.state.player_pos = predicted_pos;

        let seq = self.input_queue.push(
            action.clone(),
            Some(input_queue::PredictedState {
                pos: predicted_pos,
            }),
        );

        self.network
            .send_message(&ClientMessage::PlayerAction {
                seq,
                action,
                target: None,
            })
            .await?;

        Ok(predicted_pos)
    }

    pub async fn send_wait(&mut self) -> Result<()> {
        let seq = self.input_queue.push(ActionType::Wait, None);
        self.network
            .send_message(&ClientMessage::PlayerAction {
                seq,
                action: ActionType::Wait,
                target: None,
            })
            .await?;
        Ok(())
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        self.network.send_message(&ClientMessage::Logout).await?;
        self.network.close();
        Ok(())
    }
}
