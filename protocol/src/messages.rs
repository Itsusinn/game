use serde::{Deserialize, Serialize};

use crate::types::{Coord, EntityData, ItemStack, LogEntry, TileData};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Coord, EntityData, ItemStack, LogEntry, TileData, TileFlags};
    use serde::de::DeserializeOwned;
    use std::fmt::Debug;

    fn roundtrip<T>(v: T)
    where
        T: Serialize + DeserializeOwned + PartialEq + Debug,
    {
        let bytes = rmp_serde::to_vec(&v).expect("serialize");
        let back: T = rmp_serde::from_slice(&bytes).expect("deserialize");
        assert_eq!(v, back);
    }

    fn sample_tile() -> TileData {
        TileData {
            pos: Coord::new(1, 2),
            tile_type: 1,
            flags: TileFlags(0b0011),
            fg_color: 0xFFAA_BBCC,
            bg_color: 0x1122_3344,
        }
    }

    fn sample_entity() -> EntityData {
        EntityData {
            id: 7,
            entity_type: 1,
            name: "goblin".to_string(),
            pos: Coord::new(3, 4),
            hp: 8,
            max_hp: 12,
            is_player: false,
        }
    }

    fn sample_log() -> LogEntry {
        LogEntry {
            text: "you hit the rat for 4 dmg".to_string(),
            color: 0xFF00_00FF,
            turn: 42,
        }
    }

    // ClientMessage variants

    #[test]
    fn client_login() {
        roundtrip(ClientMessage::Login {
            version: PROTOCOL_VERSION,
            player_name: "alice".to_string(),
        });
    }

    #[test]
    fn client_player_action() {
        roundtrip(ClientMessage::PlayerAction {
            seq: 5,
            action: ActionType::MoveUpRight,
            target: Some(Coord::new(10, 20)),
        });
    }

    #[test]
    fn client_logout() {
        roundtrip(ClientMessage::Logout);
    }

    #[test]
    fn client_ping() {
        roundtrip(ClientMessage::Ping { seq: 99 });
    }

    #[test]
    fn action_chat() {
        roundtrip(ActionType::Chat {
            text: "hello".to_string(),
        });
    }

    // ServerMessage variants

    #[test]
    fn server_login_accepted() {
        roundtrip(ServerMessage::LoginAccepted {
            player_id: 1,
            sub_world_id: (3, -7),
            world_seed: 0xDEAD_BEEF,
        });
    }

    #[test]
    fn server_world_state() {
        roundtrip(ServerMessage::WorldState {
            seq: 12,
            player_pos: Coord::new(100, 200),
            visible_tiles: vec![sample_tile()],
            explored: vec![true, false, true],
            explored_width: 3,
            entities: vec![sample_entity()],
            items_ground: vec![(
                Coord::new(5, 6),
                ItemStack {
                    item_id: 2,
                    quantity: 3,
                },
            )],
            message_log: vec![sample_log()],
            hp: 90,
            stamina: 80,
            thirst: 70,
            hunger: 60,
        });
    }

    #[test]
    fn server_entity_moved() {
        roundtrip(ServerMessage::EntityMoved {
            id: 1,
            from: Coord::new(0, 0),
            to: Coord::new(1, 0),
        });
    }

    #[test]
    fn server_attack_result() {
        roundtrip(ServerMessage::AttackResult {
            attacker: 1,
            target: 2,
            damage: 7,
            killed: false,
        });
    }

    #[test]
    fn server_chat_message() {
        roundtrip(ServerMessage::ChatMessage {
            player_id: 1,
            player_name: "bob".to_string(),
            text: "hi".to_string(),
        });
    }

    #[test]
    fn server_sub_world_transfer() {
        roundtrip(ServerMessage::SubWorldTransfer {
            new_sub_world_id: (1, -1),
            pos: Coord::new(256, 256),
        });
    }

    #[test]
    fn server_entity_joined() {
        roundtrip(ServerMessage::EntityJoined {
            entity: sample_entity(),
        });
    }

    #[test]
    fn server_entity_left() {
        roundtrip(ServerMessage::EntityLeft { id: 7 });
    }

    #[test]
    fn server_chunk_data() {
        roundtrip(ServerMessage::ChunkData {
            cx: 1,
            cy: 2,
            tiles: vec![sample_tile(), sample_tile()],
        });
    }

    #[test]
    fn server_error() {
        roundtrip(ServerMessage::Error {
            code: 1,
            text: "bad".to_string(),
        });
    }

    #[test]
    fn server_pong() {
        roundtrip(ServerMessage::Pong { seq: 5 });
    }
}
