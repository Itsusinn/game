use protocol::TileFlags;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileType {
    Floor,
    Wall,
    Door { open: bool },
    Water,
    StairsUp,
    StairsDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tile {
    pub tile_type: TileType,
    pub flags: TileFlags,
}

impl Tile {
    pub fn new(tile_type: TileType) -> Self {
        let flags = match tile_type {
            TileType::Wall => TileFlags::new(TileFlags::BLOCKS_MOVEMENT | TileFlags::BLOCKS_VISION),
            TileType::Door { open: false } => {
                TileFlags::new(TileFlags::BLOCKS_MOVEMENT | TileFlags::BLOCKS_VISION)
            }
            TileType::Door { open: true } => TileFlags::new(0),
            TileType::Water => TileFlags::new(TileFlags::BLOCKS_MOVEMENT | TileFlags::IS_WATER),
            _ => TileFlags::new(0),
        };
        Self { tile_type, flags }
    }

    pub fn is_passable(&self) -> bool {
        matches!(self.tile_type, TileType::Door { open: true }) || !self.flags.blocks_movement()
    }

    pub fn blocks_vision(&self) -> bool {
        !matches!(self.tile_type, TileType::Door { open: true }) && self.flags.blocks_vision()
    }

    pub fn fg_color(&self) -> u32 {
        match self.tile_type {
            TileType::Floor => 0x888888,
            TileType::Wall => 0x444444,
            TileType::Door { .. } => 0x8B4513,
            TileType::Water => 0x0000FF,
            TileType::StairsUp => 0xFFFF00,
            TileType::StairsDown => 0xFF8800,
        }
    }

    pub fn bg_color(&self) -> u32 {
        0x000000
    }
}
