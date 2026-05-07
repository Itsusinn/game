use crate::world::tile::{Tile, TileType};
use protocol::SUBWORLD_SIZE;

pub struct GameMap {
    tiles: Vec<Tile>,
    pub explored: Vec<bool>,
}

impl GameMap {
    pub fn new() -> Self {
        let count = (SUBWORLD_SIZE as usize) * (SUBWORLD_SIZE as usize);
        Self {
            tiles: vec![Tile::new(TileType::Wall); count],
            explored: vec![false; count],
        }
    }

    pub(crate) fn index(&self, x: i32, y: i32) -> usize {
        (y as usize) * (SUBWORLD_SIZE as usize) + (x as usize)
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && x < SUBWORLD_SIZE && y >= 0 && y < SUBWORLD_SIZE
    }

    pub fn get_tile(&self, x: i32, y: i32) -> Option<&Tile> {
        if self.in_bounds(x, y) {
            Some(&self.tiles[self.index(x, y)])
        } else {
            None
        }
    }

    pub fn set_tile(&mut self, x: i32, y: i32, tile: Tile) {
        if self.in_bounds(x, y) {
            let idx = self.index(x, y);
            self.tiles[idx] = tile;
        }
    }

    pub fn set_explored(&mut self, x: i32, y: i32, value: bool) {
        if self.in_bounds(x, y) {
            let idx = self.index(x, y);
            self.explored[idx] = value;
        }
    }

    pub fn is_passable(&self, x: i32, y: i32) -> bool {
        self.get_tile(x, y).map(|t| t.is_passable()).unwrap_or(false)
    }

    pub fn blocks_vision(&self, x: i32, y: i32) -> bool {
        self.get_tile(x, y)
            .map(|t| t.blocks_vision())
            .unwrap_or(true)
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, tile_type: TileType) {
        for dy in 0..h {
            for dx in 0..w {
                self.set_tile(x + dx, y + dy, Tile::new(tile_type));
            }
        }
    }

    pub fn width(&self) -> i32 {
        SUBWORLD_SIZE
    }

    pub fn height(&self) -> i32 {
        SUBWORLD_SIZE
    }

    pub fn tiles_in_radius(
        &self,
        cx: i32,
        cy: i32,
        radius: i32,
    ) -> Vec<(i32, i32, &Tile)> {
        let mut result = Vec::new();
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let x = cx + dx;
                let y = cy + dy;
                if let Some(tile) = self.get_tile(x, y) {
                    result.push((x, y, tile));
                }
            }
        }
        result
    }
}
