use protocol::{ClientMessage, ServerMessage};

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
