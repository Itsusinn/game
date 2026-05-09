use std::sync::mpsc;
use tokio::sync::mpsc as tokio_mpsc;
use tokio::sync::oneshot;

use godot::builtin::{Dictionary, Variant};
use godot::prelude::*;

use crate::input_queue::{InputQueue, PredictedState};
use crate::prediction::{check_rollback, predict_move};
use crate::state::GameState;
use protocol::*;

#[derive(GodotClass)]
#[class(base=Node)]
pub struct CddaClient {
    #[base]
    base: Base<Node>,

    recv_rx: Option<mpsc::Receiver<ServerMessage>>,
    send_tx: Option<tokio_mpsc::Sender<ClientMessage>>,
    shutdown_tx: Option<oneshot::Sender<()>>,

    state: GameState,
    input_queue: InputQueue,
    connected: bool,
}

#[godot_api]
impl CddaClient {
    #[func]
    fn connect_to_game_server(&mut self, addr: GString) {
        let addr: std::net::SocketAddr = match addr.to_string().parse() {
            Ok(a) => a,
            Err(e) => {
                godot_print!("Invalid address: {e}");
                return;
            }
        };

        let (send_tx, mut send_rx) = tokio_mpsc::channel::<ClientMessage>(64);
        let (recv_tx, recv_rx) = mpsc::channel::<ServerMessage>();
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

        self.send_tx = Some(send_tx);
        self.recv_rx = Some(recv_rx);
        self.shutdown_tx = Some(shutdown_tx);

        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("network: failed to create runtime: {e}");
                    let _ = recv_tx.send(ServerMessage::Error {
                        code: 2,
                        text: format!("runtime init failed: {e}"),
                    });
                    return;
                }
            };

            rt.block_on(async {
                let mut net = match crate::network_client::NetworkClient::connect(addr).await {
                    Ok(n) => n,
                    Err(e) => {
                        eprintln!("network: connect failed: {e}");
                        let _ = recv_tx.send(ServerMessage::Error {
                            code: 1,
                            text: format!("Connection failed: {e}"),
                        });
                        return;
                    }
                };

                let login = ClientMessage::Login {
                    version: 1,
                    player_name: "godot_player".to_string(),
                };
                if let Err(e) = net.send_message(&login).await {
                    eprintln!("network: login send failed: {e}");
                    let _ = recv_tx.send(ServerMessage::Error {
                        code: 2,
                        text: format!("login send failed: {e}"),
                    });
                    return;
                }

                loop {
                    tokio::select! {
                        result = net.recv_message() => {
                            match result {
                                Ok(msg) => {
                                    if let Err(e) = recv_tx.send(msg) {
                                        eprintln!("network: recv channel closed: {e}");
                                        break;
                                    }
                                }
                                Err(e) => {
                                    eprintln!("network: recv failed: {e}");
                                    let _ = recv_tx.send(ServerMessage::Error {
                                        code: 2,
                                        text: format!("disconnect: {e}"),
                                    });
                                    break;
                                }
                            }
                        }
                        cmd = send_rx.recv() => {
                            match cmd {
                                Some(msg) => {
                                    if let Err(e) = net.send_message(&msg).await {
                                        eprintln!("network: send failed: {e}");
                                        let _ = recv_tx.send(ServerMessage::Error {
                                            code: 2,
                                            text: format!("send failed: {e}"),
                                        });
                                        break;
                                    }
                                }
                                None => {
                                    eprintln!("network: send channel closed");
                                    break;
                                }
                            }
                        }
                        _ = &mut shutdown_rx => {
                            let _ = net.send_message(&ClientMessage::Logout).await;
                            net.close();
                            break;
                        }
                    }
                }
            });
        });

        self.connected = true;
    }

    #[func]
    fn tick(&mut self, _delta: f64) {
        if !self.connected {
            return;
        }

        while let Some(rx) = &self.recv_rx {
            match rx.try_recv() {
                Ok(msg) => self.handle_message(msg),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.connected = false;
                    break;
                }
            }
        }
    }

    #[func]
    fn send_move(&mut self, direction: i32) {
        if !self.connected {
            return;
        }

        let action = match direction {
            0 => ActionType::MoveUp,
            1 => ActionType::MoveDown,
            2 => ActionType::MoveLeft,
            3 => ActionType::MoveRight,
            4 => ActionType::MoveUpLeft,
            5 => ActionType::MoveUpRight,
            6 => ActionType::MoveDownLeft,
            7 => ActionType::MoveDownRight,
            _ => return,
        };

        let predicted_pos = predict_move(self.state.player_pos, &action);
        self.state.player_pos = predicted_pos;

        let seq = self.input_queue.push(
            action.clone(),
            Some(PredictedState {
                pos: predicted_pos,
            }),
        );

        if let Some(tx) = &self.send_tx {
            if let Err(e) = tx.blocking_send(ClientMessage::PlayerAction {
                seq,
                action,
                target: None,
            }) {
                godot_error!("dropped move action (channel closed): {}", e);
            }
        }
    }

    #[func]
    fn send_wait(&mut self) {
        if !self.connected {
            return;
        }
        let seq = self.input_queue.push(ActionType::Wait, None);
        if let Some(tx) = &self.send_tx {
            if let Err(e) = tx.blocking_send(ClientMessage::PlayerAction {
                seq,
                action: ActionType::Wait,
                target: None,
            }) {
                godot_error!("dropped wait action (channel closed): {}", e);
            }
        }
    }

    #[func]
    fn get_player_pos(&self) -> Vector2i {
        Vector2i {
            x: self.state.player_pos.x,
            y: self.state.player_pos.y,
        }
    }

    #[func]
    fn get_player_hp(&self) -> i32 { self.state.hp }
    #[func]
    fn get_player_max_hp(&self) -> i32 { self.state.max_hp }
    #[func]
    fn get_player_stamina(&self) -> i32 { self.state.stamina }
    #[func]
    fn get_player_hunger(&self) -> i32 { self.state.hunger }
    #[func]
    fn get_player_thirst(&self) -> i32 { self.state.thirst }

    #[func]
    fn get_entity_count(&self) -> i32 {
        self.state.entities.len() as i32
    }

    #[func]
    fn get_visible_tile_count(&self) -> i32 {
        self.state.visible_tiles.len() as i32
    }

    #[func]
    fn get_visible_tile(&self, index: i32) -> Dictionary<Variant, Variant> {
        let mut dict = Dictionary::new();
        if index >= 0 && (index as usize) < self.state.visible_tiles.len() {
            let t = &self.state.visible_tiles[index as usize];
            dict.set("pos_x", t.pos.x);
            dict.set("pos_y", t.pos.y);
            dict.set("tile_type", t.tile_type as i32);
            dict.set("fg_color", t.fg_color as i32);
            dict.set("bg_color", t.bg_color as i32);
        }
        dict
    }

    #[func]
    fn get_entity(&self, index: i32) -> Dictionary<Variant, Variant> {
        let mut dict = Dictionary::new();
        if index >= 0 && (index as usize) < self.state.entities.len() {
            let e = &self.state.entities[index as usize];
            dict.set("id", e.id);
            dict.set("pos_x", e.pos.x);
            dict.set("pos_y", e.pos.y);
            dict.set("name", e.name.as_str());
            dict.set("entity_type", e.entity_type as i32);
            dict.set("hp", e.hp);
            dict.set("max_hp", e.max_hp);
            dict.set("is_player", e.is_player);
        }
        dict
    }

    #[func]
    fn get_log_count(&self) -> i32 {
        self.state.message_log.len() as i32
    }

    #[func]
    fn get_log_entry(&self, index: i32) -> Dictionary<Variant, Variant> {
        let mut dict = Dictionary::new();
        if index >= 0 && (index as usize) < self.state.message_log.len() {
            let entry = &self.state.message_log[index as usize];
            dict.set("text", entry.text.as_str());
            dict.set("color", entry.color as i32);
        }
        dict
    }

    #[func]
    fn disconnect_from_server(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        self.send_tx = None;
        self.connected = false;
    }

    #[func]
    fn is_game_connected(&self) -> bool {
        self.connected
    }
}

#[godot_api]
impl INode for CddaClient {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            recv_rx: None,
            send_tx: None,
            shutdown_tx: None,
            state: GameState::default(),
            input_queue: InputQueue::new(),
            connected: false,
        }
    }
}

impl CddaClient {
    fn handle_message(&mut self, msg: ServerMessage) {
        match &msg {
            ServerMessage::WorldState { seq, player_pos, .. } => {
                let acked = self.input_queue.ack_up_to(*seq);
                for pending in &acked {
                    if let Some(ref predicted) = pending.predicted {
                        let _ = check_rollback(predicted.pos, *player_pos);
                    }
                }
                self.state.apply_snapshot_owned(msg);
            }
            ServerMessage::Error { code, text } => {
                godot_error!("Server error ({}): {}", code, text);
                self.connected = false;
            }
            _ => {
                self.state.apply_delta(&msg);
            }
        }
    }
}
