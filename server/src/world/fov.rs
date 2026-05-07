use std::collections::HashSet;

use crate::world::map::GameMap;
use protocol::Coord;

pub fn compute_fov(map: &GameMap, origin: Coord, radius: i32, visible: &mut HashSet<Coord>) {
    visible.clear();
    visible.insert(origin);

    for octant in 0..8 {
        cast_light(map, origin.x, origin.y, radius, 1, 1.0, 0.0, octant, visible);
    }
}

fn cast_light(
    map: &GameMap,
    cx: i32,
    cy: i32,
    radius: i32,
    row: i32,
    mut start_slope: f64,
    end_slope: f64,
    octant: u8,
    visible: &mut HashSet<Coord>,
) {
    if start_slope < end_slope {
        return;
    }

    let mut next_start_slope = start_slope;

    for j in row..=radius {
        let mut blocked = false;
        let dy = -j;

        for dx in dy..=0 {
            let left_slope = (dx as f64 - 0.5) / (dy as f64 + 0.5);
            let right_slope = (dx as f64 + 0.5) / (dy as f64 - 0.5);

            if start_slope < right_slope {
                continue;
            }
            if end_slope > left_slope {
                break;
            }

            let (sx, sy) = transform_octant(cx, cy, dx, -dy, octant);

            if dx * dx + dy * dy <= radius * radius {
                visible.insert(Coord::new(sx, sy));
            }

            if blocked {
                if map.blocks_vision(sx, sy) {
                    next_start_slope = right_slope;
                    continue;
                }
                blocked = false;
                start_slope = next_start_slope;
            } else if map.blocks_vision(sx, sy) && j < radius {
                blocked = true;
                cast_light(
                    map,
                    cx,
                    cy,
                    radius,
                    j + 1,
                    start_slope,
                    left_slope,
                    octant,
                    visible,
                );
                next_start_slope = right_slope;
            }
        }

        if blocked {
            break;
        }
    }
}

fn transform_octant(cx: i32, cy: i32, dx: i32, dy: i32, octant: u8) -> (i32, i32) {
    match octant {
        0 => (cx + dx, cy + dy),
        1 => (cx + dy, cy + dx),
        2 => (cx + dy, cy - dx),
        3 => (cx + dx, cy - dy),
        4 => (cx - dx, cy - dy),
        5 => (cx - dy, cy - dx),
        6 => (cx - dy, cy + dx),
        7 => (cx - dx, cy + dy),
        _ => (cx, cy),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::map::GameMap;
    use crate::world::tile::{Tile, TileType};
    use protocol::Coord;

    #[test]
    fn test_fov_empty_room() {
        let mut map = GameMap::new();
        // Make a 20x20 room of floor tiles
        for y in 0..20 {
            for x in 0..20 {
                map.set_tile(x, y, Tile::new(TileType::Floor));
            }
        }

        let origin = Coord::new(10, 10);
        let mut visible = HashSet::new();
        compute_fov(&map, origin, 8, &mut visible);

        assert!(visible.contains(&origin));
        assert!(visible.len() > 100); // Should see most tiles in radius
    }

    #[test]
    fn test_fov_blocked_by_wall() {
        let mut map = GameMap::new();
        // Floor everywhere
        for y in 0..20 {
            for x in 0..20 {
                map.set_tile(x, y, Tile::new(TileType::Floor));
            }
        }
        // Wall at (12, 10)
        map.set_tile(12, 10, Tile::new(TileType::Wall));

        let origin = Coord::new(10, 10);
        let mut visible = HashSet::new();
        compute_fov(&map, origin, 10, &mut visible);

        // (12, 10) itself should be visible (the wall)
        assert!(visible.contains(&Coord::new(12, 10)));
        // But tiles behind the wall should not be visible
        assert!(!visible.contains(&Coord::new(15, 10)));
    }
}
