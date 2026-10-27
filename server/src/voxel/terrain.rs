use game_simulation::grid::Grid;
use protocol::*;

/// Generate a complete column using world coordinates, independent of visit order.
/// The original 64x64 authored area is retained by generate()/legacy migration.
pub(super) fn ensure_column(map: &mut Grid, seed: u64, c: [i32; 2]) {
    if map.has_column(c) || !valid_column(c) {
        return;
    }
    let [ox, oz] = c.map(|v| v * 16);
    let mut column = Grid::unknown();
    for y in [0, 16] {
        column.apply_chunk([ox, y, oz], &vec![AIR; 4096]);
    }
    let height =
        |x: i32, z: i32| 5 + ((x as f32 * 0.16).sin() * 1.5 + (z as f32 * 0.19).cos() * 1.5) as i32;
    for z in oz..oz + 16 {
        for x in ox..ox + 16 {
            let h = height(x, z);
            for y in 0..=h {
                column.set(
                    [x, y, z],
                    if y == h {
                        GRASS
                    } else if y > h - 3 {
                        DIRT
                    } else {
                        STONE
                    },
                );
            }
        }
    }
    // Examine neighboring tree roots as well, clipping writes to this column.
    for z in oz - 2..oz + 18 {
        for x in ox - 2..ox + 18 {
            // No new tree roots in the preserved, authored spawn area.
            if (0..64).contains(&x) && (0..64).contains(&z) {
                continue;
            }
            let hash = (x as i64 as u64)
                .wrapping_mul(73)
                .wrapping_add((z as i64 as u64).wrapping_mul(193))
                .wrapping_add(seed);
            if !hash.is_multiple_of(53) {
                continue;
            }
            let h = height(x, z);
            for y in h + 1..=h + 4 {
                column.set([x, y, z], WOOD);
            }
            for dy in 3..=6 {
                for dz in -2i32..=2 {
                    for dx in -2i32..=2 {
                        if dx.abs() + dz.abs() <= 3 && (dy < 6 || dx.abs() + dz.abs() <= 1) {
                            let p = [x + dx, h + dy, z + dz];
                            if column.get(p) == AIR {
                                column.set(p, LEAVES);
                            }
                        }
                    }
                }
            }
        }
    }
    for (origin, blocks) in column.blocks {
        map.apply_chunk(origin, &blocks);
    }
}

pub fn generate(seed: u64) -> Grid {
    let mut map = Grid::empty();
    for z in 0..64 {
        for x in 0..64 {
            let h = if (19..=45).contains(&x) && (17..=45).contains(&z) {
                6
            } else {
                5 + ((x as f32 * 0.16).sin() * 1.5 + (z as f32 * 0.19).cos() * 1.5) as i32
            };
            for y in 0..=h {
                map.set(
                    [x, y, z],
                    if y == h {
                        GRASS
                    } else if y > h - 3 {
                        DIRT
                    } else {
                        STONE
                    },
                );
            }
            if (x < 18 || z < 15 || x > 47 || z > 48)
                && (x as u64 * 73 + z as u64 * 193)
                    .wrapping_add(seed)
                    .is_multiple_of(53)
                && x > 3
                && z > 3
                && x < 60
                && z < 60
            {
                for y in h + 1..=h + 4 {
                    map.set([x, y, z], WOOD);
                }
                for dy in 3..=6 {
                    for dz in -2i32..=2 {
                        for dx in -2i32..=2 {
                            if dx.abs() + dz.abs() <= 3 && (dy < 6 || dx.abs() + dz.abs() <= 1) {
                                map.set([x + dx, h + dy, z + dz], LEAVES);
                            }
                        }
                    }
                }
            }
        }
    }
    // An open stone shelter, timber floor, window openings and reachable roof.
    for z in 19..=27 {
        for x in 23..=34 {
            map.set([x, 6, z], PLANK);
            if x == 23 || x == 34 || z == 19 || z == 27 {
                for y in 7..=10 {
                    let doorway = z == 27 && (28..=29).contains(&x) && y < 10;
                    let window = y == 9
                        && ((x == 23 || x == 34) && (22..=24).contains(&z)
                            || z == 19 && (26..=30).contains(&x));
                    if !doorway && !window {
                        map.set([x, y, z], STONE);
                    }
                }
            }
            if z <= 22 {
                map.set([x, 11, z], PLANK);
            }
        }
    }
    for i in 0..4 {
        for x in 35..=36 {
            for y in 7..=7 + i {
                map.set([x, y, 26 - i], STONE);
            }
        }
    }
    // Low stepping blocks and a raised platform demonstrate real vertical collision.
    for i in 0..3 {
        map.set([24 - i, 7 + i, 35], STONE);
    }
    for z in 34..=38 {
        for x in 17..=21 {
            map.set([x, 9, z], PLANK);
        }
    }
    // A shallow pool has a solid bed, no swimming system in this milestone.
    for z in 34..=39 {
        for x in 39..=44 {
            map.set([x, 6, z], WATER);
            map.set([x, 5, z], STONE);
        }
    }
    map.set([26, 7, 30], CHEST);
    map
}
