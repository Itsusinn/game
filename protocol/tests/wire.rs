use protocol::{ClientMessage, ServerMessage};

#[test]
fn inventory_slot_commands_roundtrip() {
    use protocol::*;
    for command in [
        InventoryCommand::Select { slot: 8 },
        InventoryCommand::Move {
            revision: 5,
            from: 0,
            to: 35,
            quantity: 32,
        },
        InventoryCommand::QuickMove {
            revision: 7,
            slot: 35,
        },
    ] {
        let message = ClientMessage::Inventory(command);
        assert_eq!(
            rmp_serde::from_slice::<ClientMessage>(&rmp_serde::to_vec(&message).unwrap()).unwrap(),
            message
        );
    }
    let mut inventory = PlayerInventory::default();
    inventory.slots[35] = Some(InventoryEntry {
        name: "Supplies".into(),
        quantity: 64,
    });
    inventory.selected = 8;
    inventory.revision = 17;
    assert_eq!(
        rmp_serde::from_slice::<PlayerInventory>(&rmp_serde::to_vec(&inventory).unwrap()).unwrap(),
        inventory
    );
}

#[test]
fn container_commands_and_snapshot_roundtrip() {
    use protocol::*;
    for command in [
        ContainerCommand::Open {
            position: [26, 7, 30],
        },
        ContainerCommand::Close,
        ContainerCommand::Transfer {
            position: [26, 7, 30],
            revision: 9,
            inventory_revision: 4,
            slot: 2,
            target: None,
            quantity: 2,
            deposit: true,
        },
    ] {
        let message = ClientMessage::Container(command);
        let bytes = rmp_serde::to_vec(&message).unwrap();
        assert_eq!(
            rmp_serde::from_slice::<ClientMessage>(&bytes).unwrap(),
            message
        );
    }
    let snapshot = ContainerSnapshot {
        position: [26, 7, 30],
        revision: 10,
        items: vec![InventoryEntry {
            name: "Supplies".into(),
            quantity: 2,
        }],
    };
    assert_eq!(
        rmp_serde::from_slice::<ContainerSnapshot>(&rmp_serde::to_vec(&snapshot).unwrap()).unwrap(),
        snapshot
    );
}

#[test]
fn active_messages_keep_named_wire_tags() {
    // These variants follow removed 2D variants in the old enums. Their wire
    // tags must remain names, independent of their position in the Rust enum.
    let logout = b"\xa6Logout";
    assert_eq!(rmp_serde::to_vec(&ClientMessage::Logout).unwrap(), logout);
    assert_eq!(
        rmp_serde::from_slice::<ClientMessage>(logout).unwrap(),
        ClientMessage::Logout
    );
    let pong = b"\x81\xa4Pong\x91\x05";
    assert_eq!(
        rmp_serde::to_vec(&ServerMessage::Pong { seq: 5 }).unwrap(),
        pong
    );
    assert_eq!(
        rmp_serde::from_slice::<ServerMessage>(pong).unwrap(),
        ServerMessage::Pong { seq: 5 }
    );
}
