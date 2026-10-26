use crate::mesh::{Mesh, NORMALS};
use game_client_core::voxel::Client3d;
use protocol::*;

pub fn terrain_chunk(model: &Client3d, origin: [i32; 3]) -> Mesh {
    let mut mesh = Mesh::default();
    for y in origin[1]..origin[1] + CHUNK_EDGE {
        for z in origin[2]..origin[2] + CHUNK_EDGE {
            for x in origin[0]..origin[0] + CHUNK_EDGE {
                let block = model.terrain.get([x, y, z]);
                if block == AIR || block == 255 {
                    continue;
                }
                for (face, n) in NORMALS.iter().enumerate() {
                    let neighbor =
                        model
                            .terrain
                            .get([x + n[0] as i32, y + n[1] as i32, z + n[2] as i32]);
                    if neighbor != AIR && !(neighbor == WATER && block != WATER) && neighbor != 255
                    {
                        continue;
                    }
                    if y == 0 && face == 2 {
                        continue;
                    }
                    let (color, pattern) = match block {
                        GRASS if face == 3 => ([0.46, 0.62, 0.32], 3),
                        GRASS | DIRT => ([0.48, 0.36, 0.24], 3),
                        STONE => ([0.56, 0.58, 0.53], 2),
                        WOOD => ([0.43, 0.32, 0.21], 1),
                        LEAVES => ([0.33, 0.51, 0.27], 4),
                        WATER => ([0.29, 0.49, 0.58], 0),
                        PLANK => ([0.65, 0.51, 0.33], 1),
                        _ => ([0.5, 0.5, 0.5], 0),
                    };
                    mesh.block_face(
                        [x as f32, y as f32, z as f32],
                        [1.0; 3],
                        face,
                        color,
                        pattern,
                    );
                }
            }
        }
    }
    mesh
}

pub fn actors(model: &Client3d) -> Mesh {
    let mut mesh = Mesh::default();
    for a in &model.actors {
        if a.id == model.player_id {
            continue;
        }
        let start = mesh.vertices.len();
        let skin = if a.kind == 0 {
            [0.78, 0.63, 0.47]
        } else {
            [0.45, 0.57, 0.33]
        };
        let shirt = if a.kind == 0 {
            [0.35, 0.57, 0.59]
        } else {
            [0.46, 0.35, 0.30]
        };
        mesh.cuboid([-0.22, 1.30, -0.22], [0.44, 0.44, 0.44], skin, 0);
        mesh.cuboid([-0.23, 0.62, -0.16], [0.46, 0.68, 0.32], shirt, 0);
        for x in [-0.23, 0.04] {
            mesh.cuboid([x, 0.0, -0.15], [0.19, 0.62, 0.30], [0.23, 0.29, 0.33], 0);
        }
        for x in [-0.40, 0.24] {
            mesh.cuboid([x, 0.68, -0.14], [0.16, 0.58, 0.28], shirt, 0);
            mesh.cuboid([x, 0.56, -0.14], [0.16, 0.16, 0.28], skin, 0);
        }
        mesh.cuboid(
            [-0.23, 1.65, -0.23],
            [0.46, 0.12, 0.46],
            [0.22, 0.21, 0.18],
            0,
        );
        for x in [-0.14, 0.07] {
            mesh.cuboid([x, 1.49, -0.231], [0.07, 0.06, 0.02], [0.10, 0.12, 0.10], 0);
        }
        for v in &mut mesh.vertices[start..] {
            let (s, c) = (-a.yaw).sin_cos();
            let p = v.position;
            let n = v.normal;
            v.position = [
                a.position[0] + p[0] * c + p[2] * s,
                a.position[1] + p[1],
                a.position[2] - p[0] * s + p[2] * c,
            ];
            v.normal = [n[0] * c + n[2] * s, n[1], -n[0] * s + n[2] * c];
        }
    }
    for item in &model.items {
        let p = item.position;
        mesh.cuboid(
            [p[0] - 0.18, p[1] + 0.04, p[2] - 0.15],
            [0.36, 0.40, 0.30],
            [0.74, 0.59, 0.32],
            1,
        );
        mesh.cuboid(
            [p[0] - 0.19, p[1] + 0.31, p[2] - 0.16],
            [0.38, 0.06, 0.32],
            [0.40, 0.32, 0.22],
            0,
        );
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_simulation::grid::Grid;

    #[test]
    fn adjacent_chunks_cull_their_shared_faces() {
        let mut m = Client3d::default();
        m.terrain = Grid::empty();
        m.terrain.set([15, 7, 8], STONE);
        m.terrain.set([16, 7, 8], STONE);
        let left = terrain_chunk(&m, [0, 0, 0]);
        let right = terrain_chunk(&m, [16, 0, 0]);
        assert_eq!(left.vertices.len(), 5 * 6);
        assert_eq!(right.vertices.len(), 5 * 6);
        assert!(left.vertices.iter().all(|v| v.normal != [1.0, 0.0, 0.0]));
        assert!(right.vertices.iter().all(|v| v.normal != [-1.0, 0.0, 0.0]));
    }
}
