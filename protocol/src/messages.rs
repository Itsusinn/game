use serde::{Deserialize, Serialize};

use crate::types::{Coord, EntityData, ItemStack, LogEntry, TileData};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    Login {
        version: u32,
        player_name: String,
    },
    PlayerAction {
        seq: u32,
        action: ActionType,
        target: Option<Coord>,
    },
    Logout,
    Ping {
        seq: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionType {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    MoveUpLeft,
    MoveUpRight,
    MoveDownLeft,
    MoveDownRight,
    Wait,
    MeleeAttack,
    RangedAttack,
    Pickup,
    Drop,
    UseItem,
    Craft,
    Inspect,
    Interact,
    Chat { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    LoginAccepted {
        player_id: u32,
        sub_world_id: (i64, i64),
        world_seed: u64,
    },
    WorldState {
        seq: u32,
        player_pos: Coord,
        visible_tiles: Vec<TileData>,
        explored: Vec<bool>,
        explored_width: u32,
        entities: Vec<EntityData>,
        items_ground: Vec<(Coord, ItemStack)>,
        message_log: Vec<LogEntry>,
        hp: i32,
        stamina: i32,
        thirst: i32,
        hunger: i32,
    },
    EntityMoved {
        id: u32,
        from: Coord,
        to: Coord,
    },
    AttackResult {
        attacker: u32,
        target: u32,
        damage: i32,
        killed: bool,
    },
    ChatMessage {
        player_id: u32,
        player_name: String,
        text: String,
    },
    SubWorldTransfer {
        new_sub_world_id: (i64, i64),
        pos: Coord,
    },
    EntityJoined {
        entity: EntityData,
    },
    EntityLeft {
        id: u32,
    },
    ChunkData {
        cx: i32,
        cy: i32,
        tiles: Vec<TileData>,
    },
    Error {
        code: u32,
        text: String,
    },
    Pong {
        seq: u32,
    },
}
