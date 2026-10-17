use crate::network_client::{NetworkEvent, QuicSession, Transport};
use game_client_core::voxel::Client3d;
use game_simulation::{grid::Grid, step_with_actors};
use protocol::*;

pub struct Session {
    pub model: Client3d,
    pub transport: Option<Box<dyn Transport>>,
    pub address: String,
    pub name: String,
    pub certificate: String,
    pub status: String,
    pub demo: bool,
    pub generation: u64,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            model: Client3d::default(),
            transport: None,
            address: "127.0.0.1:9876".into(),
            name: "Explorer".into(),
            certificate: "save/server.cert.der".into(),
            status: "Connect to explore the 3D world.".into(),
            demo: false,
            generation: 0,
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
            Ok(t) => {
                self.transport = Some(Box::new(t));
                self.status = "Connecting...".into();
            }
            Err(e) => self.status = format!("Connection failed: {e:#}"),
        }
    }
    pub fn disconnect(&mut self) {
        self.transport = None;
        self.model = Client3d::default();
        self.demo = false;
        self.generation += 1;
        self.status = "Disconnected".into();
    }
    pub fn shutdown(&mut self) {
        if let Some(transport) = &mut self.transport {
            transport.shutdown();
        }
        self.disconnect();
    }
    pub fn tick(&mut self) {
        let mut disconnected = false;
        if let Some(t) = &mut self.transport {
            for _ in 0..256 {
                match t.poll() {
                    Some(NetworkEvent::Message(m)) => {
                        self.model.apply(m);
                        if self.model.ready {
                            self.status = "Connected / 3D world".into();
                        }
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
            self.model.ready = false;
        }
    }
    pub fn input(&mut self, input: PlayerInput) {
        if self.demo {
            let obstacles: Vec<_> = self
                .model
                .actors
                .iter()
                .filter(|a| a.id != self.model.player_id)
                .map(|a| a.position)
                .collect();
            step_with_actors(
                &mut self.model.body,
                input,
                |p| self.model.terrain.solid(p),
                &obstacles,
            );
        } else if let Some(t) = &self.transport {
            if let Err(e) = self.model.input(input, |m| t.send(m)) {
                self.status = e;
            }
        }
    }
    pub fn chat(&mut self, text: String) {
        if let Some(t) = &self.transport {
            if let Err(e) = t.send(ClientMessage::Chat3d { text }) {
                self.status = e;
            }
        }
    }
    pub fn load_demo(&mut self) {
        self.disconnect();
        self.demo = true;
        self.status = "Offline 3D walkthrough / connect for combat and multiplayer".into();
        let mut grid = Grid::empty();
        for z in 0..64 {
            for x in 0..64 {
                for y in 0..=6 {
                    grid.set([x, y, z], if y == 6 { GRASS } else { DIRT });
                }
            }
        }
        for z in 18..=26 {
            for x in 23..=34 {
                grid.set([x, 6, z], PLANK);
                if x == 23 || x == 34 || z == 18 {
                    for y in 7..11 {
                        grid.set([x, y, z], STONE);
                    }
                }
                if z < 21 {
                    grid.set([x, 11, z], PLANK);
                }
            }
        }
        for i in 0..4 {
            for y in 7..=7 + i {
                grid.set([36, y, 27 - i], STONE);
            }
        }
        for (x, z) in [(20, 30), (40, 20), (42, 35), (20, 17), (15, 26), (37, 14)] {
            for y in 7..12 {
                grid.set([x, y, z], WOOD);
            }
            for y in 10..14 {
                for dx in -2..=2 {
                    for dz in -2..=2 {
                        grid.set([x + dx, y, z + dz], LEAVES);
                    }
                }
            }
        }
        self.model.apply(ServerMessage::Welcome3d {
            player_id: 1,
            seed: 0,
        });
        for origin in Grid::origins() {
            self.model.apply(ServerMessage::Chunk3d {
                origin,
                blocks: grid.chunk(origin),
            });
        }
        self.model.apply(ServerMessage::World3d(Snapshot3d {
            seq: 0,
            body: Body {
                position: [28.5, 7.0, 32.5],
                grounded: true,
                ..Default::default()
            },
            actors: vec![Actor3d {
                id: 2,
                kind: 1,
                position: [31.5, 7.0, 25.5],
                yaw: 0.0,
                hp: 30,
                max_hp: 30,
            }],
            items: vec![GroundItem3d {
                id: 1,
                position: [27.0, 7.0, 29.0],
                name: "Supplies".into(),
            }],
            hp: 100,
            inventory: vec![],
            log: vec![],
        }));
    }
}
