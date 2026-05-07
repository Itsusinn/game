use crate::world::entity::{Entity, EntityManager};
use crate::world::map::GameMap;
use crate::world::pathfinding;
use protocol::{ActionType, Coord};

pub fn ai_action(
    entity: &Entity,
    map: &GameMap,
    entities: &EntityManager,
    player_pos: Coord,
    alert_radius: i32,
    chase_radius: i32,
) -> Option<ActionType> {
    let dist = heuristic(entity.pos, player_pos);

    // Attack: if adjacent to player
    if dist <= 1 {
        return Some(ActionType::MeleeAttack);
    }

    // Chase: if player is within chase radius, move toward them
    if dist <= chase_radius {
        if let Some(next) =
            pathfinding::find_step_toward(map, entity.pos, player_pos, entities)
        {
            let dx = next.x - entity.pos.x;
            let dy = next.y - entity.pos.y;
            return Some(dir_to_action(dx, dy));
        }
    }

    // Alert: random wandering if in alert radius
    if dist <= alert_radius {
        // Random step
        let dirs = [(-1, 0), (1, 0), (0, -1), (0, 1)];
        // Use entity id as simple deterministic random seed per turn
        let idx = ((entity.id as i32).wrapping_mul(7) % dirs.len() as i32) as usize;
        let (dx, dy) = dirs[idx];
        let nx = entity.pos.x + dx;
        let ny = entity.pos.y + dy;
        if map.is_passable(nx, ny) && entities.entities_in_radius(nx, ny, 0).is_empty() {
            return Some(dir_to_action(dx, dy));
        }
    }

    // Idle: do nothing
    None
}

fn dir_to_action(dx: i32, dy: i32) -> ActionType {
    match (dx.signum(), dy.signum()) {
        (-1, -1) => ActionType::MoveUpLeft,
        (0, -1) => ActionType::MoveUp,
        (1, -1) => ActionType::MoveUpRight,
        (-1, 0) => ActionType::MoveLeft,
        (1, 0) => ActionType::MoveRight,
        (-1, 1) => ActionType::MoveDownLeft,
        (0, 1) => ActionType::MoveDown,
        (1, 1) => ActionType::MoveDownRight,
        _ => ActionType::Wait,
    }
}

fn heuristic(a: Coord, b: Coord) -> i32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}
