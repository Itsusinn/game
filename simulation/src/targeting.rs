//! Shared voxel targeting for authoritative edits and crosshair feedback.
use crate::{grid::Grid, EYE_HEIGHT};
use protocol::{AIR, BLOCK_REACH, WATER};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockHit {
    pub position: [i32; 3],
    pub normal: [i32; 3],
}

pub fn target_block(map: &Grid, feet: [f32; 3], yaw: f32, pitch: f32) -> Option<BlockHit> {
    if !feet.iter().all(|v| v.is_finite())
        || !yaw.is_finite()
        || !pitch.is_finite()
        || yaw.abs() > std::f32::consts::TAU
        || pitch.abs() > 1.5
    {
        return None;
    }
    let eye = [feet[0], feet[1] + EYE_HEIGHT, feet[2]];
    let direction = [
        yaw.sin() * pitch.cos(),
        pitch.sin(),
        -yaw.cos() * pitch.cos(),
    ];
    let mut cell = eye.map(|v| v.floor() as i32);
    Grid::index(cell)?;
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
    let mut normal = [0; 3];
    for _ in 0..32 {
        match map.get(cell) {
            255 => return None,
            AIR | WATER => {}
            _ => {
                return Some(BlockHit {
                    position: cell,
                    normal,
                })
            }
        }
        let axis = (0..3).min_by(|&a, &b| next[a].total_cmp(&next[b]))?;
        if next[axis] > BLOCK_REACH {
            return None;
        }
        cell[axis] += step[axis];
        normal = [0; 3];
        normal[axis] = -step[axis];
        next[axis] += delta[axis];
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::STONE;
    #[test]
    fn first_hit_face_range_unknown_and_invalid_angles() {
        let mut map = Grid::empty();
        map.set([10, 2, 7], STONE);
        let feet = [10.5, 1.0, 10.5];
        assert_eq!(
            target_block(&map, feet, 0.0, 0.0),
            Some(BlockHit {
                position: [10, 2, 7],
                normal: [0, 0, 1],
            })
        );
        map.set([10, 2, 9], STONE);
        assert_eq!(
            target_block(&map, feet, 0.0, 0.0).unwrap().position,
            [10, 2, 9]
        );
        map.set([10, 2, 9], 255);
        assert!(target_block(&map, feet, 0.0, 0.0).is_none());
        assert!(target_block(&map, feet, f32::NAN, 0.0).is_none());
        assert!(target_block(&map, feet, 0.0, f32::INFINITY).is_none());
        assert!(target_block(&map, [10.5, 1.0, 15.0], 0.0, 0.0).is_none());
        assert!(target_block(&map, [-1.0, 1.0, 10.0], 0.0, 0.0).is_none());
    }
}
