use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use tracing::{debug, instrument};

use crate::world::entity::{EntityManager, EntityType};
use crate::world::item::{ItemManager, ItemType};
use crate::world::map::GameMap;
use crate::world::tile::{Tile, TileType};
use protocol::{Coord, SUBWORLD_SIZE};

const MIN_ROOM_SIZE: i32 = 4;
const MAX_ROOM_SIZE: i32 = 10;
const MIN_ROOMS: usize = 8;
const MAX_ROOMS: usize = 16;
const MAX_ATTEMPTS: usize = 200;

#[instrument(level = "info", skip(map, entities, items), fields(sw_id = ?sw_id))]
pub fn generate_sub_world(
    map: &mut GameMap,
    entities: &mut EntityManager,
    items: &mut ItemManager,
    world_seed: u64,
    sw_id: (i64, i64),
) -> Coord {
    let seed = world_seed
        .wrapping_add((sw_id.0 as u64).wrapping_mul(6364136223846793005))
        .wrapping_add((sw_id.1 as u64).wrapping_mul(1442695040888963407));
    let mut rng = StdRng::seed_from_u64(seed);

    let num_rooms = rng.gen_range(MIN_ROOMS..=MAX_ROOMS);
    let mut rooms: Vec<(i32, i32, i32, i32)> = Vec::new();

    for _ in 0..MAX_ATTEMPTS {
        if rooms.len() >= num_rooms {
            break;
        }
        let w = rng.gen_range(MIN_ROOM_SIZE..=MAX_ROOM_SIZE);
        let h = rng.gen_range(MIN_ROOM_SIZE..=MAX_ROOM_SIZE);
        let x = rng.gen_range(2..SUBWORLD_SIZE - w - 2);
        let y = rng.gen_range(2..SUBWORLD_SIZE - h - 2);

        let collides = rooms.iter().any(|(rx, ry, rw, rh)| {
            x < rx + rw + 2 && x + w + 2 > *rx && y < ry + rh + 2 && y + h + 2 > *ry
        });

        if !collides {
            map.fill_rect(x, y, w, h, TileType::Floor);
            rooms.push((x, y, w, h));
        }
    }
    debug!(actual = rooms.len(), target = num_rooms, "Rooms generated");

    for i in 1..rooms.len() {
        let (x1, y1, w1, h1) = rooms[i - 1];
        let (x2, y2, w2, h2) = rooms[i];
        let cx1 = x1 + w1 / 2;
        let cy1 = y1 + h1 / 2;
        let cx2 = x2 + w2 / 2;
        let cy2 = y2 + h2 / 2;

        if rng.gen_bool(0.5) {
            carve_h_corridor(map, cx1, cx2, cy1);
            carve_v_corridor(map, cy1, cy2, cx2);
        } else {
            carve_v_corridor(map, cy1, cy2, cx1);
            carve_h_corridor(map, cx1, cx2, cy2);
        }

        if rng.gen_bool(0.4) {
            let door_x = cx1 + rng.gen_range(-1..=1);
            let door_y = cy1 + rng.gen_range(-1..=1);
            if map.in_bounds(door_x, door_y) && map.is_passable(door_x, door_y) {
                map.set_tile(door_x, door_y, Tile::new(TileType::Door { open: false }));
            }
        }
    }

    for (rx, ry, rw, rh) in &rooms {
        if *rw * *rh < 15 {
            continue;
        }
        let count = rng.gen_range(0..=3);
        for _ in 0..count {
            let ex = rng.gen_range(*rx + 1..rx + rw - 1);
            let ey = rng.gen_range(*ry + 1..ry + rh - 1);
            let (name, etype, hp) = match rng.gen_range(0..4) {
                0 => ("Zombie".into(), EntityType::Zombie, 15),
                1 => ("Skeleton".into(), EntityType::Skeleton, 12),
                2 => ("Goblin".into(), EntityType::Goblin, 8),
                _ => ("Rat".into(), EntityType::Rat, 4),
            };
            entities.spawn(name, etype, Coord::new(ex, ey), hp);
        }

        // Spawn ground items
        let item_count = rng.gen_range(0..=2);
        for _ in 0..item_count {
            let ix = rng.gen_range(*rx + 1..rx + rw - 1);
            let iy = rng.gen_range(*ry + 1..ry + rh - 1);
            let (item_name, item_type) = match rng.gen_range(0..5) {
                0 => ("Short Sword", ItemType::Weapon { damage: 5, attack_bonus: 1 }),
                1 => ("Leather Armor", ItemType::Armor { defense: 2 }),
                2 => ("Health Potion", ItemType::Potion { heal: 20 }),
                3 => ("Bread", ItemType::Food { nutrition: 30 }),
                _ => ("Scroll of Light", ItemType::Scroll { effect: "light".into() }),
            };
            items.spawn_ground(item_name, item_type, Coord::new(ix, iy), 1);
        }
    }

    if !rooms.is_empty() {
        let (sx, sy, sw, sh) = rooms[0];
        map.set_tile(
            sx + sw / 2,
            sy + sh / 2,
            Tile::new(TileType::StairsDown),
        );
    }
    if rooms.len() > 1 {
        let (sx, sy, sw, sh) = rooms[rooms.len() - 1];
        map.set_tile(
            sx + sw / 2 + 1,
            sy + sh / 2,
            Tile::new(TileType::StairsUp),
        );
    }

    if let Some((rx, ry, rw, rh)) = rooms.first() {
        Coord::new(rx + rw / 2, ry + rh / 2)
    } else {
        Coord::new(SUBWORLD_SIZE / 2, SUBWORLD_SIZE / 2)
    }
}

fn carve_h_corridor(map: &mut GameMap, x1: i32, x2: i32, y: i32) {
    let start = x1.min(x2);
    let end = x1.max(x2);
    for x in start..=end {
        if map.in_bounds(x, y) && !map.is_passable(x, y) {
            map.set_tile(x, y, Tile::new(TileType::Floor));
        }
    }
}

fn carve_v_corridor(map: &mut GameMap, y1: i32, y2: i32, x: i32) {
    let start = y1.min(y2);
    let end = y1.max(y2);
    for y in start..=end {
        if map.in_bounds(x, y) && !map.is_passable(x, y) {
            map.set_tile(x, y, Tile::new(TileType::Floor));
        }
    }
}
