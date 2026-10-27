//! Deterministic, host-independent character movement shared by authority and prediction.
use protocol::{Body, PlayerInput};
pub mod grid;
mod physics;
pub mod targeting;

pub const DT: f32 = 1.0 / 30.0;
pub const RADIUS: f32 = 0.29;
pub const HEIGHT: f32 = 1.75;
pub const EYE_HEIGHT: f32 = 1.58;
pub const SPEED: f32 = 4.2;

pub fn overlaps(position: [f32; 3], solid: &impl Fn([i32; 3]) -> bool) -> bool {
    let min = [
        position[0] - RADIUS,
        position[1] + 0.001,
        position[2] - RADIUS,
    ];
    let max = [
        position[0] + RADIUS,
        position[1] + HEIGHT - 0.001,
        position[2] + RADIUS,
    ];
    for y in min[1].floor() as i32..=max[1].floor() as i32 {
        for z in min[2].floor() as i32..=max[2].floor() as i32 {
            for x in min[0].floor() as i32..=max[0].floor() as i32 {
                if solid([x, y, z]) {
                    return true;
                }
            }
        }
    }
    false
}

pub fn step(body: &mut Body, input: PlayerInput, solid: impl Fn([i32; 3]) -> bool) {
    step_with_actors(body, input, solid, &[]);
}

pub fn step_with_actors(
    body: &mut Body,
    input: PlayerInput,
    solid: impl Fn([i32; 3]) -> bool,
    actors: &[[f32; 3]],
) {
    step_player_with_actors(body, input, false, solid, actors);
}

/// Flying is an authoritative movement mode, never a client input permission.
pub fn step_player_with_actors(
    body: &mut Body,
    input: PlayerInput,
    flying: bool,
    solid: impl Fn([i32; 3]) -> bool,
    actors: &[[f32; 3]],
) {
    if !input.valid() {
        return;
    }
    let vertical = if flying {
        f32::from(input.jump) - f32::from(input.descend)
    } else {
        0.0
    };
    let length =
        (input.forward * input.forward + input.strafe * input.strafe + vertical * vertical)
            .sqrt()
            .max(1.0);
    let speed = SPEED * if flying && input.sprint { 3.0 } else { 1.0 };
    let (sin, cos) = input.yaw.sin_cos();
    body.velocity[0] = (sin * input.forward + cos * input.strafe) / length * speed;
    body.velocity[2] = (-cos * input.forward + sin * input.strafe) / length * speed;
    if flying {
        body.velocity[1] = vertical / length * speed;
    } else {
        if input.jump && body.grounded {
            body.velocity[1] = 8.2;
        }
        body.velocity[1] = (body.velocity[1] - 23.0 * DT).max(-20.0);
    }
    physics::move_character(body, &solid, actors);
}

pub fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

pub fn clear_line(a: [f32; 3], b: [f32; 3], solid: impl Fn([i32; 3]) -> bool) -> bool {
    let steps = (distance(a, b) / 0.08).ceil().max(1.0) as usize;
    (1..steps).all(|i| {
        let t = i as f32 / steps as f32;
        let p = std::array::from_fn(|j| (a[j] + (b[j] - a[j]) * t).floor() as i32);
        !solid(p)
    })
}

/// Shared interaction rule; the target voxel itself is allowed to terminate sight.
pub fn container_reachable(map: &grid::Grid, feet: [f32; 3], target: [i32; 3]) -> bool {
    let eye = [feet[0], feet[1] + EYE_HEIGHT, feet[2]];
    let center = target.map(|v| v as f32 + 0.5);
    map.get(target) == protocol::CHEST
        && distance(eye, center) <= protocol::CONTAINER_REACH
        && clear_line(eye, center, |p| p != target && map.solid(p))
}

/// Traverse voxels along the crosshair ray, stopping at the first solid block.
pub fn targeted_container(
    map: &grid::Grid,
    feet: [f32; 3],
    yaw: f32,
    pitch: f32,
) -> Option<[i32; 3]> {
    if !feet.iter().all(|v| v.is_finite()) || !yaw.is_finite() || !pitch.is_finite() {
        return None;
    }
    let eye = [feet[0], feet[1] + EYE_HEIGHT, feet[2]];
    let direction = [
        yaw.sin() * pitch.cos(),
        pitch.sin(),
        -yaw.cos() * pitch.cos(),
    ];
    let mut cell = eye.map(|v| v.floor() as i32);
    let step = direction.map(|d| if d >= 0.0 { 1 } else { -1 });
    let delta = direction.map(|d| {
        if d.abs() < 1e-6 {
            f32::INFINITY
        } else {
            1.0 / d.abs()
        }
    });
    let mut next: [f32; 3] = std::array::from_fn(|i| {
        if delta[i].is_infinite() {
            f32::INFINITY
        } else {
            ((cell[i] + i32::from(step[i] > 0)) as f32 - eye[i]) / direction[i]
        }
    });
    for _ in 0..32 {
        if map.solid(cell) {
            return container_reachable(map, feet, cell).then_some(cell);
        }
        let axis = (0..3).min_by(|&a, &b| next[a].total_cmp(&next[b]))?;
        if next[axis] > protocol::CONTAINER_REACH {
            return None;
        }
        cell[axis] += step[axis];
        next[axis] += delta[axis];
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flight_ascends_descends_hovers_accelerates_and_resumes_gravity() {
        let start = Body {
            position: [10.0, 10.0, 10.0],
            ..Default::default()
        };
        let mut b = start;
        step_player_with_actors(
            &mut b,
            PlayerInput {
                jump: true,
                ..Default::default()
            },
            true,
            |_| false,
            &[],
        );
        assert!((b.position[1] - start.position[1] - SPEED * DT).abs() < 0.001);
        let hovering = b.position;
        for _ in 0..30 {
            step_player_with_actors(&mut b, PlayerInput::default(), true, |_| false, &[]);
        }
        assert_eq!(b.position, hovering);
        step_player_with_actors(
            &mut b,
            PlayerInput {
                descend: true,
                sprint: true,
                ..Default::default()
            },
            true,
            |_| false,
            &[],
        );
        assert!((hovering[1] - b.position[1] - 3.0 * SPEED * DT).abs() < 0.001);
        b = start;
        step_player_with_actors(
            &mut b,
            PlayerInput {
                jump: true,
                strafe: 1.0,
                forward: 1.0,
                sprint: true,
                ..Default::default()
            },
            true,
            |_| false,
            &[],
        );
        assert!((distance(start.position, b.position) - 3.0 * SPEED * DT).abs() < 0.001);
        step_player_with_actors(&mut b, PlayerInput::default(), false, |_| false, &[]);
        // Switching mode on the server zeroes velocity; simulate the same transition.
        b.velocity = [0.0; 3];
        let y = b.position[1];
        step_player_with_actors(
            &mut b,
            PlayerInput {
                descend: true,
                sprint: true,
                ..Default::default()
            },
            false,
            |_| false,
            &[],
        );
        assert!(b.position[1] < y);
        assert_eq!(b.velocity[0], 0.0);
    }

    #[test]
    fn accelerated_flight_respects_walls_ceiling_actors_and_unknown_terrain() {
        let mut b = Body {
            position: [0.5, 1.0, 0.5],
            ..Default::default()
        };
        for _ in 0..30 {
            step_player_with_actors(
                &mut b,
                PlayerInput {
                    jump: true,
                    strafe: 1.0,
                    sprint: true,
                    ..Default::default()
                },
                true,
                |p| p[0] >= 2 || p[1] >= 4,
                &[],
            );
        }
        assert!(b.position[0] < 2.0 - RADIUS);
        assert!(b.position[1] < 4.0 - HEIGHT);
        b.position = [0.5, 1.0, 0.5];
        for _ in 0..30 {
            step_player_with_actors(
                &mut b,
                PlayerInput {
                    strafe: 1.0,
                    sprint: true,
                    ..Default::default()
                },
                true,
                |_| false,
                &[[2.0, 1.0, 0.5]],
            );
        }
        assert!(b.position[0] < 2.0 - 2.0 * RADIUS);
        let map = grid::Grid::empty();
        b.position = [63.0, 10.0, 10.0];
        for _ in 0..30 {
            step_player_with_actors(
                &mut b,
                PlayerInput {
                    strafe: 1.0,
                    sprint: true,
                    ..Default::default()
                },
                true,
                |p| map.solid(p),
                &[],
            );
        }
        assert!(b.position[0] < 64.0 - RADIUS);
    }
    #[test]
    fn chest_targeting_stops_at_walls_and_respects_reach() {
        let mut map = grid::Grid::empty();
        map.set([10, 7, 10], protocol::CHEST);
        let feet = [10.5, 7.0, 12.5];
        let pitch = (-1.08f32).atan2(2.0);
        assert_eq!(
            targeted_container(&map, feet, 0.0, pitch),
            Some([10, 7, 10])
        );
        assert_eq!(
            targeted_container(&map, feet, std::f32::consts::PI, pitch),
            None
        );
        map.set([10, 8, 11], protocol::STONE);
        assert_eq!(targeted_container(&map, feet, 0.0, pitch), None);
        assert!(!container_reachable(&map, feet, [10, 7, 10]));
        map.set([10, 8, 11], protocol::AIR);
        assert!(!container_reachable(&map, [10.5, 7.0, 15.5], [10, 7, 10]));
        assert!(map.solid([10, 7, 10]));
    }
    fn ground(p: [i32; 3]) -> bool {
        p[1] < 0
    }
    fn body() -> Body {
        Body {
            position: [0.5, 0.0, 0.5],
            grounded: true,
            ..Body::default()
        }
    }
    #[test]
    fn continuous_motion_and_diagonal_speed_are_bounded() {
        let mut a = body();
        let mut b = body();
        for _ in 0..30 {
            step(
                &mut a,
                PlayerInput {
                    forward: 1.0,
                    ..Default::default()
                },
                ground,
            );
            step(
                &mut b,
                PlayerInput {
                    forward: 1.0,
                    strafe: 1.0,
                    ..Default::default()
                },
                ground,
            );
        }
        assert!(
            (distance(a.position, body().position) - SPEED).abs() < 0.01,
            "{a:?} {b:?}"
        );
        assert!(
            (distance(a.position, body().position) - distance(b.position, body().position)).abs()
                < 0.01,
            "{a:?} {b:?}"
        );
    }
    #[test]
    fn jump_lands_and_ceiling_and_wall_block_motion() {
        let mut b = body();
        step(
            &mut b,
            PlayerInput {
                jump: true,
                ..Default::default()
            },
            ground,
        );
        let mut peak = b.position[1];
        for _ in 0..90 {
            step(&mut b, PlayerInput::default(), ground);
            peak = peak.max(b.position[1]);
        }
        assert!(peak > 1.1);
        assert!(b.grounded);
        assert!(b.position[1].abs() < 0.01);
        for _ in 0..60 {
            step(
                &mut b,
                PlayerInput {
                    strafe: 1.0,
                    jump: true,
                    ..Default::default()
                },
                |p| ground(p) || p[0] >= 2 || p[1] >= 2,
            );
        }
        assert!(b.position[0] < 1.72);
        assert!(b.position[1] < 0.26);
    }
    #[test]
    fn invalid_input_and_occluded_attacks_are_rejected() {
        let mut b = body();
        let old = b;
        step(
            &mut b,
            PlayerInput {
                yaw: f32::NAN,
                ..Default::default()
            },
            ground,
        );
        assert_eq!(b, old);
        assert!(!clear_line([0.5, 1.0, 0.5], [3.5, 1.0, 0.5], |p| p[0] == 2));
    }
    #[test]
    fn characters_cannot_walk_through_each_other_or_world_bounds() {
        let mut b = body();
        for _ in 0..60 {
            step_with_actors(
                &mut b,
                PlayerInput {
                    strafe: 1.0,
                    ..Default::default()
                },
                ground,
                &[[2.0, 0.0, 0.5]],
            );
        }
        assert!(b.position[0] < 1.43);
        let grid = grid::Grid::empty();
        b.position = [0.5, 10.0, 0.5];
        for _ in 0..60 {
            step(
                &mut b,
                PlayerInput {
                    strafe: -1.0,
                    ..Default::default()
                },
                |p| grid.solid(p),
            );
        }
        assert!(b.position[0] >= RADIUS - 0.001);
        assert!(b.position[1] >= -0.002);
    }
}
pub mod inventory;
