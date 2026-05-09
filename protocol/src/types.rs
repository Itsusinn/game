use serde::{Deserialize, Serialize};

pub const CHUNK_SIZE: i32 = 32;
pub const REGION_SIZE: i32 = 16;
pub const SUBWORLD_SIZE: i32 = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Coord {
    pub x: i32,
    pub y: i32,
}

impl Coord {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn sub_world_id(&self) -> (i64, i64) {
        (
            self.x.div_euclid(SUBWORLD_SIZE as i32) as i64,
            self.y.div_euclid(SUBWORLD_SIZE as i32) as i64,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileFlags(pub u8);

impl TileFlags {
    pub const BLOCKS_MOVEMENT: u8 = 1 << 0;
    pub const BLOCKS_VISION: u8 = 1 << 1;
    pub const IS_DOOR: u8 = 1 << 2;
    pub const IS_WATER: u8 = 1 << 3;

    pub const fn new(flags: u8) -> Self {
        Self(flags)
    }

    pub fn has(&self, flag: u8) -> bool {
        self.0 & flag != 0
    }

    pub fn blocks_movement(&self) -> bool {
        self.has(Self::BLOCKS_MOVEMENT)
    }

    pub fn blocks_vision(&self) -> bool {
        self.has(Self::BLOCKS_VISION)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TileData {
    pub pos: Coord,
    pub tile_type: u8,
    pub flags: TileFlags,
    pub fg_color: u32,
    pub bg_color: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityData {
    pub id: u32,
    pub entity_type: u8,
    pub name: String,
    pub pos: Coord,
    pub hp: i32,
    pub max_hp: i32,
    pub is_player: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemStack {
    pub item_id: u32,
    pub quantity: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    pub text: String,
    pub color: u32,
    pub turn: u64,
}
