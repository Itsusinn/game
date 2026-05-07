use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::world::entity::EntityManager;
use crate::world::map::GameMap;
use protocol::Coord;

#[derive(Debug, Clone, Eq, PartialEq)]
struct AStarNode {
    pos: Coord,
    g: i32,
    f: i32,
}

impl Ord for AStarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other.f.cmp(&self.f).then_with(|| other.g.cmp(&self.g))
    }
}

impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn heuristic(a: Coord, b: Coord) -> i32 {
    let dx = (a.x - b.x).abs();
    let dy = (a.y - b.y).abs();
    dx.max(dy)
}

pub fn find_path(
    map: &GameMap,
    start: Coord,
    goal: Coord,
    entities: &EntityManager,
    max_steps: i32,
) -> Option<Vec<Coord>> {
    if start == goal {
        return Some(vec![]);
    }

    let mut open = BinaryHeap::new();
    let mut came_from: HashMap<(i32, i32), Coord> = HashMap::new();
    let mut g_score: HashMap<(i32, i32), i32> = HashMap::new();

    open.push(AStarNode {
        pos: start,
        g: 0,
        f: heuristic(start, goal),
    });
    g_score.insert((start.x, start.y), 0);

    let directions = [
        Coord::new(0, -1),
        Coord::new(0, 1),
        Coord::new(-1, 0),
        Coord::new(1, 0),
        Coord::new(-1, -1),
        Coord::new(1, -1),
        Coord::new(-1, 1),
        Coord::new(1, 1),
    ];

    while let Some(current) = open.pop() {
        if current.pos == goal {
            // Reconstruct path
            let mut path = vec![goal];
            let mut cur = goal;
            while cur != start {
                if let Some(prev) = came_from.get(&(cur.x, cur.y)) {
                    cur = *prev;
                    path.push(cur);
                } else {
                    break;
                }
            }
            path.reverse();
            return Some(path);
        }

        let cur_g = *g_score.get(&(current.pos.x, current.pos.y)).unwrap_or(&i32::MAX);

        if cur_g >= max_steps {
            continue;
        }

        for dir in &directions {
            let nx = current.pos.x + dir.x;
            let ny = current.pos.y + dir.y;

            // Skip start position (we don't want to go back to start through a zero-length path)
            if nx == start.x && ny == start.y && cur_g > 0 {
                continue;
            }

            if !map.is_passable(nx, ny) {
                continue;
            }

            // Check if this tile is occupied (except for the goal)
            if (nx, ny) != (goal.x, goal.y)
                && !entities.entities_in_radius(nx, ny, 0).is_empty()
            {
                continue;
            }

            // Diagonal moves cost 14 (approx sqrt(2)*10), cardinal cost 10
            let move_cost = if dir.x != 0 && dir.y != 0 { 14 } else { 10 };
            let tentative_g = cur_g + move_cost;

            let neighbor_key = (nx, ny);
            let existing_g = *g_score.get(&neighbor_key).unwrap_or(&i32::MAX);

            if tentative_g < existing_g {
                came_from.insert(neighbor_key, current.pos);
                g_score.insert(neighbor_key, tentative_g);
                open.push(AStarNode {
                    pos: Coord::new(nx, ny),
                    g: tentative_g,
                    f: tentative_g + heuristic(Coord::new(nx, ny), goal),
                });
            }
        }
    }

    None
}

pub fn find_step_toward(
    map: &GameMap,
    start: Coord,
    goal: Coord,
    entities: &EntityManager,
) -> Option<Coord> {
    find_path(map, start, goal, entities, 100).and_then(|path| {
        if path.len() >= 2 {
            Some(path[1])
        } else {
            path.first().copied()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::entity::EntityManager;
    use crate::world::map::GameMap;
    use crate::world::tile::{Tile, TileType};
    use protocol::Coord;

    #[test]
    fn test_path_straight_line() {
        let mut map = GameMap::new();
        for y in 0..10 {
            for x in 0..10 {
                map.set_tile(x, y, Tile::new(TileType::Floor));
            }
        }
        let entities = EntityManager::new();

        let path = find_path(&map, Coord::new(0, 0), Coord::new(5, 0), &entities, 100);
        assert!(path.is_some());
        let path = path.unwrap();
        assert_eq!(path.last(), Some(&Coord::new(5, 0)));
    }

    #[test]
    fn test_path_blocked() {
        let mut map = GameMap::new();
        for y in 0..10 {
            for x in 0..10 {
                map.set_tile(x, y, Tile::new(TileType::Floor));
            }
        }
        // Wall blocking the direct path
        map.set_tile(3, 0, Tile::new(TileType::Wall));
        map.set_tile(3, 1, Tile::new(TileType::Wall));
        map.set_tile(3, 2, Tile::new(TileType::Wall));

        let entities = EntityManager::new();
        let path = find_path(&map, Coord::new(0, 0), Coord::new(5, 0), &entities, 100);
        assert!(path.is_some()); // Should find a way around
    }
}
