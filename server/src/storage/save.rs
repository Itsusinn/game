use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::world::map::GameMap;
use crate::world::tile::TileType;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedWorld {
    pub world_seed: u64,
    pub sub_worlds: HashMap<(i64, i64), SavedMap>,
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

pub fn save_world_to(saved: &SavedWorld, filepath: &Path) -> Result<()> {
    if let Some(parent) = filepath.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).context("create save directory")?;
        }
    }
    let data = rmp_serde::to_vec(saved).context("serialize world")?;
    std::fs::write(filepath, &data).context("write world save file")?;
    Ok(())
}

pub fn load_world_from(filepath: &Path) -> Result<SavedWorld> {
    let data = std::fs::read(filepath).context("read world save file")?;
    let saved: SavedWorld = rmp_serde::from_slice(&data).context("deserialize world")?;
    Ok(saved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::tile::Tile;

    #[test]
    fn saved_map_codec_preserves_tiles_and_explored() {
        let mut map = GameMap::new();
        map.set_tile(10, 10, Tile::new(TileType::Floor));
        map.set_tile(11, 10, Tile::new(TileType::Wall));
        map.set_tile(12, 10, Tile::new(TileType::Door { open: false }));
        map.set_explored(10, 10, true);

        let saved = SavedMap::from(&map);
        let bytes = rmp_serde::to_vec(&saved).expect("serialize");
        let back: SavedMap = rmp_serde::from_slice(&bytes).expect("deserialize");

        assert_eq!(back.width, map.width());
        assert_eq!(back.height, map.height());
        assert_eq!(back.tiles.len(), saved.tiles.len());
        assert_eq!(back.explored, saved.explored);
        // Tile (11, 10) should have been encoded as Wall (1).
        let idx = (10 * map.width() + 11) as usize;
        assert_eq!(back.tiles[idx].tile_type, 1);
    }

    #[test]
    fn test_save_load_world_roundtrip() -> Result<()> {
        let mut sub_worlds = HashMap::new();
        let mut map = GameMap::new();
        map.set_tile(0, 0, Tile::new(TileType::Floor));
        sub_worlds.insert((0, 0), SavedMap::from(&map));

        let saved = SavedWorld {
            world_seed: 0xDEAD_BEEF,
            sub_worlds,
        };

        let tmp = tempfile::NamedTempFile::new()?;
        save_world_to(&saved, tmp.path())?;
        let loaded = load_world_from(tmp.path())?;

        assert_eq!(loaded.world_seed, 0xDEAD_BEEF);
        assert!(loaded.sub_worlds.contains_key(&(0, 0)));
        Ok(())
    }
}
