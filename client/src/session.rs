use crate::local_server::{LocalEvent, LocalServer};
use crate::network_client::{NetworkEvent, QuicSession, Transport};
use game_client_core::voxel::Client3d;
pub type Model = Client3d;
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
    local_server: Option<LocalServer>,
    pub local_error: bool,
    retired_transports: Vec<Box<dyn Transport>>,
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
            local_server: None,
            local_error: false,
            retired_transports: Vec::new(),
        }
    }
}
impl Session {
    pub fn shutdown_pending(&self) -> bool {
        self.local_server.as_ref().is_some_and(|s| s.stopping)
            || !self.retired_transports.is_empty()
    }

    fn retire_transport(&mut self) {
        if let Some(mut transport) = self.transport.take() {
            transport.close();
            self.retired_transports.push(transport);
        }
    }

    pub fn local_running(&self) -> bool {
        self.local_server.is_some()
    }

    pub fn start_singleplayer(&mut self, path: std::path::PathBuf) {
        self.disconnect();
        if self.local_running() {
            return;
        }
        self.local_error = false;
        match LocalServer::start(path) {
            Ok(server) => {
                self.local_server = Some(server);
                self.status = "正在启动单人世界…".into();
            }
            Err(e) => {
                self.local_error = true;
                self.status = format!("单人游戏启动失败：{e:#}");
            }
        }
    }

    pub fn connect(&mut self) {
        self.disconnect();
        if self.local_running() {
            return;
        }
        self.local_error = false;
        let result = (|| -> anyhow::Result<QuicSession> {
            QuicSession::connect_file(
                self.address.parse()?,
                self.name.clone(),
                self.certificate.clone(),
            )
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
        if let Some(server) = &mut self.local_server {
            server.stop();
        }
        self.retire_transport();
        self.model = Client3d::default();
        self.demo = false;
        self.generation += 1;
        self.status = if self.local_running() {
            "正在保存并关闭单人世界…"
        } else {
            "Disconnected"
        }
        .into();
    }
    pub fn tick(&mut self) {
        self.retired_transports
            .retain(|transport| !transport.finished());
        if let Some(server) = &mut self.local_server {
            match server.poll() {
                Some(LocalEvent::Ready(address, certificate)) if !server.stopping => {
                    match QuicSession::connect(address, self.name.clone(), certificate) {
                        Ok(transport) => {
                            self.transport = Some(Box::new(transport));
                        }
                        Err(e) => {
                            self.status = format!("本地连接失败：{e:#}");
                            self.local_error = true;
                            server.stop();
                        }
                    }
                }
                Some(LocalEvent::Finished(result)) => {
                    self.local_server = None;
                    if let Err(e) = result {
                        self.local_error = true;
                        self.status = format!("单人世界错误：{e:#}");
                    } else if !self.local_error {
                        self.status = "单人世界已保存并关闭".into();
                    }
                }
                _ => {}
            }
        }
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
            self.retire_transport();
            self.model.ready = false;
            if let Some(server) = &mut self.local_server {
                self.local_error = true;
                server.stop();
            }
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
        if self.local_running() {
            return;
        }
        self.local_error = false;
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
