use protocol::{AIR, CHUNK_EDGE, WORLD_SIZE};

#[derive(Clone)]
pub struct Grid {
    pub blocks: Vec<u8>,
}
impl Grid {
    pub fn empty() -> Self {
        Self {
            blocks: vec![AIR; (WORLD_SIZE[0] * WORLD_SIZE[1] * WORLD_SIZE[2]) as usize],
        }
    }
    pub fn unknown() -> Self {
        Self {
            blocks: vec![255; (WORLD_SIZE[0] * WORLD_SIZE[1] * WORLD_SIZE[2]) as usize],
        }
    }
    pub fn index(p: [i32; 3]) -> Option<usize> {
        if p.iter().zip(WORLD_SIZE).any(|(&v, n)| v < 0 || v >= n) {
            return None;
        }
        Some((p[0] + WORLD_SIZE[0] * (p[2] + WORLD_SIZE[2] * p[1])) as usize)
    }
    pub fn get(&self, p: [i32; 3]) -> u8 {
        Self::index(p).map_or(255, |i| self.blocks[i])
    }
    pub fn set(&mut self, p: [i32; 3], block: u8) {
        if let Some(i) = Self::index(p) {
            self.blocks[i] = block;
        }
    }
    pub fn solid(&self, p: [i32; 3]) -> bool {
        !matches!(self.get(p), AIR | protocol::WATER)
    }
    pub fn chunk(&self, origin: [i32; 3]) -> Vec<u8> {
        let mut blocks = Vec::with_capacity(4096);
        for y in 0..CHUNK_EDGE {
            for z in 0..CHUNK_EDGE {
                for x in 0..CHUNK_EDGE {
                    blocks.push(self.get([origin[0] + x, origin[1] + y, origin[2] + z]));
                }
            }
        }
        blocks
    }
    pub fn apply_chunk(&mut self, origin: [i32; 3], blocks: &[u8]) -> bool {
        if blocks.len() != 4096
            || blocks.iter().any(|b| *b > protocol::CHEST)
            || origin
                .iter()
                .zip(WORLD_SIZE)
                .any(|(&v, n)| v < 0 || v > n - CHUNK_EDGE || v % CHUNK_EDGE != 0)
        {
            return false;
        }
        let mut i = 0;
        for y in 0..CHUNK_EDGE {
            for z in 0..CHUNK_EDGE {
                for x in 0..CHUNK_EDGE {
                    self.set([origin[0] + x, origin[1] + y, origin[2] + z], blocks[i]);
                    i += 1;
                }
            }
        }
        true
    }
    pub fn origins() -> impl Iterator<Item = [i32; 3]> {
        (0..WORLD_SIZE[1]).step_by(16).flat_map(|y| {
            (0..WORLD_SIZE[2])
                .step_by(16)
                .flat_map(move |z| (0..WORLD_SIZE[0]).step_by(16).map(move |x| [x, y, z]))
        })
    }
}
