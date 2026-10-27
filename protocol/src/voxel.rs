use serde::{Deserialize, Serialize};

pub const CHUNK_EDGE: i32 = 16;
pub const WORLD_SIZE: [i32; 3] = [64, 32, 64];
pub const AIR: u8 = 0;
pub const GRASS: u8 = 1;
pub const DIRT: u8 = 2;
pub const STONE: u8 = 3;
pub const WOOD: u8 = 4;
pub const LEAVES: u8 = 5;
pub const WATER: u8 = 6;
pub const PLANK: u8 = 7;
pub const CHEST: u8 = 8;
pub const CONTAINER_SLOTS: usize = 24;
pub const CONTAINER_STACK_LIMIT: u32 = 999;
pub const CONTAINER_REACH: f32 = 3.5;

/// Container identity is its voxel coordinate, independent of scene/ECS IDs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContainerSnapshot {
    pub position: [i32; 3],
    pub revision: u64,
    pub items: Vec<crate::InventoryEntry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ContainerCommand {
    Open {
        position: [i32; 3],
    },
    Close,
    Transfer {
        position: [i32; 3],
        revision: u64,
        inventory_revision: u64,
        slot: u8,
        target: Option<u8>,
        quantity: u32,
        deposit: bool,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Body {
    /// Feet position. Y is up; X/Z are horizontal world coordinates.
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub grounded: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlayerInput {
    pub forward: f32,
    pub strafe: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub jump: bool,
    pub attack: bool,
    pub pickup: bool,
}
impl PlayerInput {
    pub fn valid(&self) -> bool {
        self.forward.is_finite()
            && self.strafe.is_finite()
            && self.yaw.is_finite()
            && self.pitch.is_finite()
            && self.forward.abs() <= 1.0
            && self.strafe.abs() <= 1.0
            && self.yaw.abs() <= std::f32::consts::TAU
            && self.pitch.abs() <= 1.5
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Actor3d {
    pub id: u32,
    pub kind: u8,
    pub position: [f32; 3],
    pub yaw: f32,
    pub hp: i32,
    pub max_hp: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroundItem3d {
    pub id: u32,
    pub position: [f32; 3],
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot3d {
    pub seq: u32,
    pub body: Body,
    pub actors: Vec<Actor3d>,
    pub items: Vec<GroundItem3d>,
    pub hp: i32,
    pub inventory: crate::PlayerInventory,
    pub log: Vec<crate::LogEntry>,
    pub container: Option<ContainerSnapshot>,
    pub container_error: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn three_dimensional_messages_roundtrip() {
        let input = crate::ClientMessage::Input3d {
            seq: 12,
            input: PlayerInput {
                forward: 0.75,
                strafe: -0.25,
                yaw: 1.2,
                pitch: 0.3,
                jump: true,
                attack: true,
                pickup: false,
            },
        };
        let bytes = rmp_serde::to_vec(&input).unwrap();
        assert_eq!(
            rmp_serde::from_slice::<crate::ClientMessage>(&bytes).unwrap(),
            input
        );
        let snapshot = crate::ServerMessage::World3d(Snapshot3d {
            seq: 11,
            body: Body {
                position: [2.25, 9.125, 14.5],
                velocity: [0.5, -4.0, 1.0],
                grounded: false,
            },
            actors: vec![Actor3d {
                id: 1,
                kind: 0,
                position: [2.25, 9.125, 14.5],
                yaw: 1.2,
                hp: 90,
                max_hp: 100,
            }],
            items: vec![],
            hp: 90,
            inventory: crate::PlayerInventory::default(),
            log: vec![],
            container: None,
            container_error: String::new(),
        });
        let bytes = rmp_serde::to_vec(&snapshot).unwrap();
        assert_eq!(
            rmp_serde::from_slice::<crate::ServerMessage>(&bytes).unwrap(),
            snapshot
        );
        let chunk = crate::ServerMessage::Chunk3d {
            origin: [16, 16, 32],
            blocks: vec![STONE; 4096],
        };
        let bytes = rmp_serde::to_vec(&chunk).unwrap();
        assert!(bytes.len() < 1_048_576);
        assert_eq!(
            rmp_serde::from_slice::<crate::ServerMessage>(&bytes).unwrap(),
            chunk
        );
    }
}
