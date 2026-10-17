use crate::network_client::{NetworkEvent, QuicSession, Transport};
use game_client_core::model::ClientModel;
use protocol::*;

pub struct Session {
    pub model: ClientModel,
    pub transport: Option<Box<dyn Transport>>,
    pub address: String,
    pub name: String,
    pub certificate: String,
    pub status: String,
    pub demo: bool,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            model: ClientModel::default(),
            transport: None,
            address: "127.0.0.1:9876".into(),
            name: "Explorer".into(),
            certificate: "save/server.cert.der".into(),
            status: "Start the server, then connect.".into(),
            demo: false,
        }
    }
}
impl Session {
    pub fn connect(&mut self) {
        self.disconnect();
        let result = (|| -> anyhow::Result<QuicSession> {
            let cert = std::fs::read(&self.certificate)?;
            QuicSession::connect(self.address.parse()?, self.name.clone(), cert)
        })();
        match result {
            Ok(transport) => {
                self.transport = Some(Box::new(transport));
                self.status = "Connecting...".into();
            }
            Err(e) => self.status = format!("Connection failed: {e:#}"),
        }
    }
    pub fn disconnect(&mut self) {
        self.transport = None;
        self.model = ClientModel::default();
        self.demo = false;
        self.status = "Disconnected".into();
    }
    pub fn tick(&mut self) {
        let mut disconnected = false;
        if let Some(transport) = &mut self.transport {
            // Bound frame work; a noisy server must not monopolize the UI.
            for _ in 0..256 {
                match transport.poll() {
                    Some(NetworkEvent::Message(message)) => {
                        match &message {
                            ServerMessage::Error { text, .. } => self.status = text.clone(),
                            ServerMessage::WorldState { .. } => self.status = "Connected".into(),
                            _ => {}
                        }
                        self.model.apply(message);
                    }
                    Some(NetworkEvent::Disconnected(reason)) => {
                        self.status = reason;
                        disconnected = true;
                        break;
                    }
                    None => break,
                }
            }
        }
        if disconnected {
            self.transport = None;
            self.model.logged_in = false;
        }
    }
    pub fn action(&mut self, action: ActionType) {
        if let Some(transport) = &self.transport {
            if let Err(error) = self
                .model
                .try_action(action, None, |message| transport.send(message))
            {
                self.status = error;
            }
        }
    }
    pub fn load_demo(&mut self) {
        self.disconnect();
        self.demo = true;
        self.status = "Renderer demo - connect to play".into();
        let mut tiles = Vec::new();
        for y in -10..=10 {
            for x in -15..=15 {
                let wall = x == -15 || x == 15 || y == -10 || y == 10 || (x == 5 && y != 0);
                let kind = if wall {
                    1
                } else if x == 5 && y == 0 {
                    2
                } else if x == -11 && y == -7 {
                    5
                } else if x == 11 && y == 7 {
                    6
                } else {
                    0
                };
                tiles.push(TileData {
                    pos: Coord::new(x, y),
                    tile_type: kind,
                    flags: TileFlags(if wall { 3 } else { 0 }),
                    fg_color: 0,
                    bg_color: 0,
                });
            }
        }
        self.model.apply(ServerMessage::WorldState {
            seq: 0,
            player_pos: Coord::new(0, 0),
            visible_tiles: tiles,
            explored: vec![],
            explored_width: 31,
            entities: vec![
                EntityData {
                    id: 1,
                    entity_type: 0,
                    name: "Explorer".into(),
                    pos: Coord::new(0, 0),
                    hp: 100,
                    max_hp: 100,
                    is_player: true,
                },
                EntityData {
                    id: 2,
                    entity_type: 3,
                    name: "Goblin".into(),
                    pos: Coord::new(8, 3),
                    hp: 8,
                    max_hp: 8,
                    is_player: false,
                },
                EntityData {
                    id: 3,
                    entity_type: 1,
                    name: "Zombie".into(),
                    pos: Coord::new(-7, -4),
                    hp: 15,
                    max_hp: 15,
                    is_player: false,
                },
            ],
            items_ground: vec![(
                Coord::new(-3, 2),
                ItemStack {
                    item_id: 1,
                    quantity: 1,
                },
            )],
            message_log: vec![LogEntry {
                text: "A quiet chamber beneath the surface.".into(),
                color: 0xA5BAC9,
                turn: 0,
            }],
            hp: 100,
            max_hp: 100,
            inventory: vec![],
            stamina: 100,
            hunger: 100,
            thirst: 100,
        });
        self.model.state.player_id = 1;
        self.model
            .visible
            .retain(|p| p.x > -11 && p.x < 12 && p.y > -8 && p.y < 9);
    }
}
