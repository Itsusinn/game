use game_client_core::model::ClientModel;
use game_engine::{Camera, Primitive};

pub fn primitives(model: &ClientModel, camera: &Camera, screen: [u32; 2]) -> Vec<Primitive> {
    let mut output = Vec::new();
    let size = camera.pixels_per_tile;
    let mut tiles: Vec<_> = model.known_tiles.values().collect();
    tiles.sort_unstable_by_key(|t| (t.pos.y, t.pos.x));
    for tile in tiles {
        let world = [tile.pos.x as f32, tile.pos.y as f32];
        if !camera.visible(world) {
            continue;
        }
        let mut color = match tile.tile_type {
            0 => [0.13, 0.18, 0.22, 1.0],
            1 => [0.43, 0.40, 0.34, 1.0],
            2 => [0.33, 0.47, 0.30, 1.0],
            3 => [0.51, 0.29, 0.21, 1.0],
            4 => [0.13, 0.33, 0.54, 1.0],
            5 => [0.83, 0.74, 0.36, 1.0],
            6 => [0.53, 0.38, 0.70, 1.0],
            _ => [0.4, 0.4, 0.4, 1.0],
        };
        if !model.visible.contains(&tile.pos) {
            let gray = (color[0] + color[1] + color[2]) / 3.0;
            for channel in &mut color[..3] {
                *channel = (*channel * 0.45 + gray * 0.55) * 0.35;
            }
        }
        let p = camera.project(world);
        let shape = match tile.tile_type {
            2 | 3 => 5,
            5 => 3,
            6 => 4,
            _ => 2,
        };
        output.push(Primitive::new(
            [p[0], p[1], size, size],
            color,
            shape,
            screen,
        ));
    }
    for (position, _) in &model.state.items_ground {
        let p = camera.project([position.x as f32 + 0.35, position.y as f32 + 0.35]);
        output.push(Primitive::new(
            [p[0], p[1], size * 0.3, size * 0.3],
            [0.9, 0.72, 0.32, 1.0],
            0,
            screen,
        ));
    }
    for entity in &model.state.entities {
        let position = if entity.id == model.state.player_id {
            model.state.player_pos
        } else {
            entity.pos
        };
        let p = camera.project([position.x as f32 + 0.14, position.y as f32 + 0.14]);
        let color = match entity.entity_type {
            0 => [0.38, 0.9, 0.65, 1.0],
            1 => [0.85, 0.3, 0.27, 1.0],
            2 => [0.9, 0.9, 0.8, 1.0],
            3 => [0.45, 0.72, 0.35, 1.0],
            _ => [0.64, 0.45, 0.29, 1.0],
        };
        output.push(Primitive::new(
            [p[0], p[1], size * 0.72, size * 0.72],
            color,
            1,
            screen,
        ));
        if entity.hp < entity.max_hp {
            output.push(Primitive::new(
                [p[0], p[1] - size * 0.15, size * 0.72, size * 0.08],
                [0.16, 0.07, 0.07, 1.0],
                0,
                screen,
            ));
            output.push(Primitive::new(
                [
                    p[0],
                    p[1] - size * 0.15,
                    size * 0.72 * (entity.hp.max(0) as f32 / entity.max_hp.max(1) as f32),
                    size * 0.08,
                ],
                [0.85, 0.3, 0.27, 1.0],
                0,
                screen,
            ));
        }
    }
    output
}
