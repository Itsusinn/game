use serde::{Deserialize, Serialize};

pub const CHUNK_EDGE: i32 = 16;
/// Legacy spawn area's dimensions; not the streaming world's horizontal bounds.
pub const WORLD_SIZE: [i32; 3] = [64, 32, 64];
pub const WORLD_HEIGHT: i32 = 128;
pub const SECTIONS_PER_COLUMN: usize = (WORLD_HEIGHT / CHUNK_EDGE) as usize;
pub const MAX_LOADED_SECTIONS: usize = 49 * SECTIONS_PER_COLUMN;
/// Conservative f32 physics/rendering precision limit.
pub const WORLD_LIMIT: i32 = 32_768;
pub const VIEW_RADIUS: i32 = 3;
pub fn valid_voxel(p: [i32; 3]) -> bool {
    (-WORLD_LIMIT..WORLD_LIMIT).contains(&p[0])
        && (0..WORLD_HEIGHT).contains(&p[1])
        && (-WORLD_LIMIT..WORLD_LIMIT).contains(&p[2])
}
pub fn valid_position(p: [f32; 3]) -> bool {
    p.iter().all(|v| v.is_finite()) && valid_voxel(p.map(|v| v.floor() as i32))
}
pub fn column_at(p: [f32; 3]) -> [i32; 2] {
    [p[0], p[2]].map(|v| (v / CHUNK_EDGE as f32).floor() as i32)
}
pub fn valid_column(c: [i32; 2]) -> bool {
    c.iter()
        .all(|v| (-WORLD_LIMIT / CHUNK_EDGE..WORLD_LIMIT / CHUNK_EDGE).contains(v))
}
pub const AIR: u8 = 0;
pub const GRASS: u8 = 1;
pub const DIRT: u8 = 2;
pub const STONE: u8 = 3;
pub const WOOD: u8 = 4;
pub const LEAVES: u8 = 5;
pub const WATER: u8 = 6;
pub const PLANK: u8 = 7;
pub const CHEST: u8 = 8;
pub const SAND: u8 = 9;
pub const SANDSTONE: u8 = 10;
pub const GRAVEL: u8 = 11;
pub const CLAY: u8 = 12;
pub const SNOW: u8 = 13;
pub const ICE: u8 = 14;
pub const MOSS_STONE: u8 = 15;
pub const BASALT: u8 = 16;
pub const BIRCH_WOOD: u8 = 17;
pub const BIRCH_LEAVES: u8 = 18;
pub const PINE_WOOD: u8 = 19;
pub const PINE_LEAVES: u8 = 20;
pub const COAL_ORE: u8 = 21;
pub const IRON_ORE: u8 = 22;
pub const COPPER_ORE: u8 = 23;
/// Append-only IDs: wire values, archive palette names and inventory names stay stable.
pub const BLOCK_IDS: [&str; 24] = [
    "game:air",
    "game:grass",
    "game:dirt",
    "game:stone",
    "game:wood",
    "game:leaves",
    "game:water",
    "game:plank",
    "game:chest",
    "game:sand",
    "game:sandstone",
    "game:gravel",
    "game:clay",
    "game:snow",
    "game:ice",
    "game:moss_stone",
    "game:basalt",
    "game:birch_wood",
    "game:birch_leaves",
    "game:pine_wood",
    "game:pine_leaves",
    "game:coal_ore",
    "game:iron_ore",
    "game:copper_ore",
];
pub fn valid_block(block: u8) -> bool {
    (block as usize) < BLOCK_IDS.len()
}
pub const CONTAINER_SLOTS: usize = 24;
pub const CONTAINER_STACK_LIMIT: u32 = 999;
pub const CONTAINER_REACH: f32 = 3.5;
pub const BLOCK_REACH: f32 = 4.5;

/// Names are also the persistent item identifiers used by the existing inventory.
pub fn block_item_name(block: u8) -> Option<&'static str> {
    match block {
        GRASS => Some("草方块"),
        DIRT => Some("泥土"),
        STONE => Some("石块"),
        WOOD => Some("原木"),
        LEAVES => Some("树叶"),
        PLANK => Some("木板"),
        CHEST => Some("箱子"),
        SAND => Some("沙子"),
        SANDSTONE => Some("砂岩"),
        GRAVEL => Some("砾石"),
        CLAY => Some("黏土"),
        SNOW => Some("雪块"),
        ICE => Some("冰块"),
        MOSS_STONE => Some("苔石"),
        BASALT => Some("玄武岩"),
        BIRCH_WOOD => Some("白桦原木"),
        BIRCH_LEAVES => Some("白桦树叶"),
        PINE_WOOD => Some("松木"),
        PINE_LEAVES => Some("松针叶"),
        COAL_ORE => Some("煤矿石"),
        IRON_ORE => Some("铁矿石"),
        COPPER_ORE => Some("铜矿石"),
        _ => None,
    }
}

pub fn item_block(name: &str) -> Option<u8> {
    (GRASS..BLOCK_IDS.len() as u8).find(|&block| block_item_name(block) == Some(name))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlockEdit {
    /// The first block hit by the crosshair, not an arbitrary placement position.
    pub target: [i32; 3],
    pub expected: u8,
    pub yaw: f32,
    pub pitch: f32,
    pub place: bool,
    pub slot: u8,
    pub inventory_revision: u64,
}

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
    pub descend: bool,
    pub sprint: bool,
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
    pub flying: bool,
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
                descend: true,
                sprint: true,
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
            flying: true,
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
