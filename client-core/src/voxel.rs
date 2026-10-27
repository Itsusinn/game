use game_simulation::{grid::Grid, step_player_with_actors};
use protocol::*;
use std::collections::{BTreeSet, VecDeque};

pub struct Client3d {
    pub terrain: Grid,
    pub revision: u64,
    pub dirty_chunks: BTreeSet<[i32; 3]>,
    pub player_id: u32,
    pub spawn_rotation: [f32; 2],
    pub body: Body,
    pub flying: bool,
    pub actors: Vec<Actor3d>,
    pub items: Vec<GroundItem3d>,
    pub hp: i32,
    pub inventory: PlayerInventory,
    pub log: Vec<LogEntry>,
    pub container: Option<ContainerSnapshot>,
    pub container_error: String,
    pub ready: bool,
    pub pending: VecDeque<(u32, PlayerInput)>,
    next_seq: u32,
    last_ack: u32,
}
impl Default for Client3d {
    fn default() -> Self {
        Self {
            terrain: Grid::unknown(),
            revision: 0,
            dirty_chunks: BTreeSet::new(),
            player_id: 0,
            spawn_rotation: [0.0, -0.1],
            body: Body::default(),
            flying: false,
            actors: vec![],
            items: vec![],
            hp: 100,
            inventory: PlayerInventory::default(),
            log: vec![],
            container: None,
            container_error: String::new(),
            ready: false,
            pending: VecDeque::new(),
            next_seq: 1,
            last_ack: 0,
        }
    }
}
impl Client3d {
    fn dirty_section(&mut self, origin: [i32; 3]) {
        self.dirty_chunks.insert(origin);
        for axis in 0..3 {
            for step in [-CHUNK_EDGE, CHUNK_EDGE] {
                let mut adjacent = origin;
                adjacent[axis] += step;
                if protocol::valid_voxel(adjacent) {
                    self.dirty_chunks.insert(adjacent);
                }
            }
        }
    }
    pub fn apply(&mut self, message: ServerMessage) {
        match message {
            ServerMessage::UnloadColumn { column } => {
                if !valid_column(column) {
                    return;
                }
                self.terrain.unload_column(column);
                self.revision += 1;
                for y in (0..WORLD_HEIGHT).step_by(16) {
                    self.dirty_section([column[0] * 16, y, column[1] * 16]);
                }
            }
            ServerMessage::Welcome3d {
                player_id,
                rotation,
                ..
            } => {
                *self = Self::default();
                self.player_id = player_id;
                self.spawn_rotation = rotation;
            }
            ServerMessage::Chunk3d { origin, blocks } => {
                if self.terrain.apply_chunk(origin, &blocks) {
                    self.revision += 1;
                    self.dirty_section(origin);
                }
            }
            ServerMessage::World3d(s) => {
                if s.seq < self.last_ack {
                    return;
                }
                self.last_ack = s.seq;
                while self.pending.front().is_some_and(|(seq, _)| *seq <= s.seq) {
                    self.pending.pop_front();
                }
                self.body = s.body;
                self.flying = s.flying;
                self.actors = s.actors;
                self.items = s.items;
                self.hp = s.hp;
                self.inventory = s.inventory;
                self.log = s.log;
                self.container = s.container;
                self.container_error = s.container_error;
                self.ready = self.player_id != 0
                    && (self.ready || self.terrain.nearby_known(s.body.position));
                let obstacles: Vec<_> = self
                    .actors
                    .iter()
                    .filter(|a| a.id != self.player_id)
                    .map(|a| a.position)
                    .collect();
                for &(_, input) in &self.pending {
                    if self.terrain.nearby_known(self.body.position) {
                        step_player_with_actors(
                            &mut self.body,
                            input,
                            self.flying,
                            |p| self.terrain.solid(p),
                            &obstacles,
                        );
                    }
                }
            }
            _ => {}
        }
    }
    pub fn input(
        &mut self,
        input: PlayerInput,
        send: impl FnOnce(ClientMessage) -> Result<(), String>,
    ) -> Result<(), String> {
        if !self.ready {
            return Err("Waiting for world data".into());
        }
        if !input.valid() || self.pending.len() >= 64 || self.next_seq == u32::MAX {
            return Err("Input queue unavailable".into());
        }
        let seq = self.next_seq;
        send(ClientMessage::Input3d { seq, input })?;
        self.next_seq += 1;
        self.pending.push_back((seq, input));
        let obstacles: Vec<_> = self
            .actors
            .iter()
            .filter(|a| a.id != self.player_id)
            .map(|a| a.position)
            .collect();
        if self.terrain.nearby_known(self.body.position) {
            step_player_with_actors(
                &mut self.body,
                input,
                self.flying,
                |p| self.terrain.solid(p),
                &obstacles,
            );
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn model() -> Client3d {
        let mut m = Client3d {
            ready: true,
            terrain: Grid::empty(),
            player_id: 1,
            ..Default::default()
        };
        for x in 0..64 {
            for z in 0..64 {
                m.terrain.set([x, 0, z], STONE);
            }
        }
        m.body = Body {
            position: [10.5, 1.0, 10.5],
            grounded: true,
            ..Default::default()
        };
        m
    }
    #[test]
    fn authority_switches_flight_before_replay_and_unknown_teleport_waits_for_chunks() {
        let mut m = model();
        let input = PlayerInput {
            jump: true,
            sprint: true,
            ..Default::default()
        };
        m.input(input, |_| Ok(())).unwrap();
        let mut s = Snapshot3d {
            seq: 0,
            body: Body {
                position: [24.0, 15.0, 24.0],
                ..Default::default()
            },
            flying: true,
            actors: vec![],
            items: vec![],
            hp: 100,
            inventory: PlayerInventory::default(),
            log: vec![],
            container: None,
            container_error: String::new(),
        };
        let mut expected = s.body;
        step_player_with_actors(&mut expected, input, true, |p| m.terrain.solid(p), &[]);
        m.apply(ServerMessage::World3d(s.clone()));
        assert!(m.flying);
        assert_eq!(m.body, expected);
        assert_eq!(m.pending.len(), 1);
        s.body.position = [-500.0, 20.0, 500.0];
        m.apply(ServerMessage::World3d(s.clone()));
        assert_eq!(
            m.body, s.body,
            "unknown destination must not resolve a body inside unknown solid cells"
        );
        m.input(input, |_| Ok(())).unwrap();
        assert_eq!(m.body, s.body);
        let c = column_at(s.body.position);
        for z in -1..=1 {
            for x in -1..=1 {
                for y in (0..WORLD_HEIGHT).step_by(16) {
                    m.apply(ServerMessage::Chunk3d {
                        origin: [(c[0] + x) * 16, y, (c[1] + z) * 16],
                        blocks: vec![AIR; 4096],
                    });
                }
            }
        }
        s.seq = 1;
        m.apply(ServerMessage::World3d(s.clone()));
        assert!(m.body.position[1] > s.body.position[1]);
        assert_eq!(m.pending.len(), 1);
        s.seq = 2;
        s.flying = false;
        m.apply(ServerMessage::World3d(s));
        assert!(!m.flying);
        assert!(m.pending.is_empty());
    }

    #[test]
    fn failed_queue_does_not_predict_and_ack_replays_remaining_input() {
        let mut m = model();
        let old = m.body;
        let input = PlayerInput {
            forward: 1.0,
            ..Default::default()
        };
        assert!(m.input(input, |_| Err("full".into())).is_err());
        assert_eq!(m.body, old);
        assert!(m.pending.is_empty());
        m.input(input, |_| Ok(())).unwrap();
        let authoritative = m.body;
        m.input(input, |_| Ok(())).unwrap();
        let predicted = m.body;
        m.apply(ServerMessage::World3d(Snapshot3d {
            flying: false,
            seq: 1,
            body: authoritative,
            actors: vec![],
            items: vec![],
            hp: 100,
            inventory: PlayerInventory::default(),
            log: vec![],
            container: None,
            container_error: String::new(),
        }));
        assert_eq!(m.body, predicted);
        assert_eq!(m.pending.len(), 1);
    }
    #[test]
    fn unknown_space_blocks_prediction_and_bad_chunks_do_not_unlock_it() {
        let mut m = Client3d::default();
        let revision = m.revision;
        m.apply(ServerMessage::Chunk3d {
            origin: [-15, 0, 0],
            blocks: vec![AIR; 4096],
        });
        assert_eq!(m.revision, revision);
        assert!(m.terrain.solid([10, 10, 10]));
        assert!(!m.ready);
    }

    #[test]
    fn unload_removes_collision_data_and_meshes_and_new_session_discards_cache() {
        let mut m = model();
        m.apply(ServerMessage::Chunk3d {
            origin: [-16, 0, 0],
            blocks: vec![AIR; 4096],
        });
        assert!(!m.terrain.solid([-1, 1, 1]));
        m.dirty_chunks.clear();
        m.apply(ServerMessage::UnloadColumn { column: [-1, 0] });
        assert!(m.terrain.solid([-1, 1, 1]));
        assert!(m.dirty_chunks.contains(&[-16, 0, 0]));
        assert!(m.dirty_chunks.contains(&[0, 0, 0]));
        m.apply(ServerMessage::Welcome3d {
            player_id: 7,
            seed: 1,
            rotation: [0.0, 0.0],
        });
        assert!(m.terrain.blocks.is_empty());
        assert!(!m.ready);
    }

    #[test]
    fn changed_chunk_updates_collision_and_invalidates_adjacent_faces_only() {
        let mut m = model();
        m.terrain.set([15, 2, 8], STONE);
        let mut updated = m.terrain.clone();
        updated.set([15, 2, 8], AIR);
        m.apply(ServerMessage::Chunk3d {
            origin: [0, 0, 0],
            blocks: updated.chunk([0, 0, 0]),
        });
        assert!(!m.terrain.solid([15, 2, 8]));
        assert_eq!(
            m.dirty_chunks,
            BTreeSet::from([
                [0, 0, 0],
                [-16, 0, 0],
                [0, 0, -16],
                [16, 0, 0],
                [0, 16, 0],
                [0, 0, 16]
            ])
        );
        assert!(!m.dirty_chunks.contains(&[48, 0, 48]));
    }
}
