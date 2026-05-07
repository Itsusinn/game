use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::world::map::GameMap;
use crate::world::tile::{Tile, TileType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedMap {
    pub width: i32,
    pub height: i32,
    pub tiles: Vec<SavedTile>,
    pub explored: Vec<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedTile {
    pub tile_type: u8,
    pub flags: u8,
}

impl From<&GameMap> for SavedMap {
    fn from(map: &GameMap) -> Self {
        let mut tiles = Vec::new();
        for y in 0..map.height() {
            for x in 0..map.width() {
                if let Some(tile) = map.get_tile(x, y) {
                    tiles.push(SavedTile {
                        tile_type: match tile.tile_type {
                            TileType::Floor => 0,
                            TileType::Wall => 1,
                            TileType::Door { open } => if open { 2 } else { 3 },
                            TileType::Water => 4,
                            TileType::StairsUp => 5,
                            TileType::StairsDown => 6,
                        },
                        flags: tile.flags.0,
                    });
                }
            }
        }

        SavedMap {
            width: map.width(),
            height: map.height(),
            tiles,
            explored: map.explored.clone(),
        }
    }
}

pub fn save_world(map: &GameMap, filepath: &Path) -> Result<()> {
    let saved = SavedMap::from(map);
    let data = rmp_serde::to_vec(&saved).context("serialize map")?;
    std::fs::write(filepath, &data).context("write save file")?;
    Ok(())
}

pub fn load_world(filepath: &Path) -> Result<GameMap> {
    let data = std::fs::read(filepath).context("read save file")?;
    let saved: SavedMap = rmp_serde::from_slice(&data).context("deserialize map")?;

    let mut map = GameMap::new();
    for (i, saved_tile) in saved.tiles.iter().enumerate() {
        let x = (i % saved.width as usize) as i32;
        let y = (i / saved.width as usize) as i32;

        let tile_type = match saved_tile.tile_type {
            0 => TileType::Floor,
            1 => TileType::Wall,
            2 => TileType::Door { open: true },
            3 => TileType::Door { open: false },
            4 => TileType::Water,
            5 => TileType::StairsUp,
            6 => TileType::StairsDown,
            _ => TileType::Floor,
        };

        map.set_tile(x, y, Tile::new(tile_type));
    }

    for (i, &explored) in saved.explored.iter().enumerate() {
        let x = (i % saved.width as usize) as i32;
        let y = (i / saved.width as usize) as i32;
        map.set_explored(x, y, explored);
    }

    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::tile::Tile;

    #[test]
    fn test_save_load_roundtrip() {
        let mut map = GameMap::new();
        map.set_tile(10, 10, Tile::new(TileType::Floor));
        map.set_tile(11, 10, Tile::new(TileType::Wall));
        map.set_tile(12, 10, Tile::new(TileType::Door { open: false }));
        map.set_explored(10, 10, true);

        let path = std::path::Path::new("/tmp/test_save.bin");
        save_world(&map, path).unwrap();

        let loaded = load_world(path).unwrap();

        assert!(loaded.is_passable(10, 10));
        assert!(!loaded.is_passable(11, 10));
        let door = loaded.get_tile(12, 10).unwrap();
        assert!(matches!(door.tile_type, TileType::Door { open: false }));
        assert!(loaded.explored[loaded.index(10, 10)]);

        std::fs::remove_file(path).ok();
    }
}
