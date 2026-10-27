use client::network_client::{
    connect_endpoint, read_message, write_message, NetworkEvent, QuicSession, Transport,
};
use protocol::*;
use server::{
    network::{make_server_endpoint, serve_endpoint},
    voxel::VoxelWorld,
};
use std::{sync::Arc, time::Duration};
use tokio::sync::{oneshot, Mutex};

async fn snapshot(recv: &mut quinn::RecvStream, seq: u32) -> Snapshot3d {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            if let ServerMessage::World3d(s) = read_message(recv).await.unwrap() {
                if s.seq >= seq {
                    return s;
                }
            }
        }
    })
    .await
    .expect("3D snapshot timed out")
}
async fn login(send: &mut quinn::SendStream, recv: &mut quinn::RecvStream, name: &str) -> u32 {
    login_uuid(send, recv, name, *uuid::Uuid::new_v4().as_bytes()).await
}
async fn login_uuid(
    send: &mut quinn::SendStream,
    recv: &mut quinn::RecvStream,
    name: &str,
    player_uuid: [u8; 16],
) -> u32 {
    write_message(
        send,
        &ClientMessage::Login {
            version: PROTOCOL_VERSION,
            player_name: name.into(),
            player_uuid,
        },
    )
    .await
    .unwrap();
    match read_message(recv).await.unwrap() {
        ServerMessage::Welcome3d { player_id, .. } => player_id,
        other => panic!("expected 3D welcome, got {other:?}"),
    }
}

#[tokio::test]
async fn stable_uuid_reconnects_after_rename_and_rejects_duplicate_online_identity() {
    let (endpoint, cert) = make_server_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let address = endpoint.local_addr().unwrap();
    let manager = Arc::new(Mutex::new(VoxelWorld::new(12345)));
    let (stop, stopped) = oneshot::channel();
    let server = tokio::spawn(serve_endpoint(endpoint, manager.clone(), async {
        let _ = stopped.await;
    }));
    let (_e1, c1) = connect_endpoint(address, cert.clone()).await.unwrap();
    let (mut s1, mut r1) = c1.open_bi().await.unwrap();
    let first = login_uuid(&mut s1, &mut r1, "Original", [5; 16]).await;
    snapshot(&mut r1, 0).await;
    write_message(
        &mut s1,
        &ClientMessage::Inventory(InventoryCommand::Select { slot: 7 }),
    )
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while snapshot(&mut r1, 0).await.inventory.selected != 7 {}
    })
    .await
    .unwrap();
    let (_e2, c2) = connect_endpoint(address, cert.clone()).await.unwrap();
    let (mut s2, mut r2) = c2.open_bi().await.unwrap();
    write_message(
        &mut s2,
        &ClientMessage::Login {
            version: PROTOCOL_VERSION,
            player_name: "Duplicate".into(),
            player_uuid: [5; 16],
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(3), read_message(&mut r2))
            .await
            .unwrap()
            .unwrap(),
        ServerMessage::Error { code: 409, .. }
    ));
    assert_eq!(manager.lock().await.player_count(), 1);
    c1.close(0u32.into(), b"reconnect");
    tokio::time::timeout(Duration::from_secs(3), async {
        while manager.lock().await.player_count() != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let (_e3, c3) = connect_endpoint(address, cert.clone()).await.unwrap();
    let (mut s3, mut r3) = c3.open_bi().await.unwrap();
    let second = login_uuid(&mut s3, &mut r3, "Renamed", [5; 16]).await;
    assert_ne!(
        first, second,
        "connection IDs remain separate from persistent UUID"
    );
    assert_eq!(snapshot(&mut r3, 0).await.inventory.selected, 7);
    let (_e4, c4) = connect_endpoint(address, cert).await.unwrap();
    let (mut s4, mut r4) = c4.open_bi().await.unwrap();
    login_uuid(&mut s4, &mut r4, "Renamed", [6; 16]).await;
    assert_eq!(
        snapshot(&mut r4, 0).await.inventory.selected,
        0,
        "same nickname with different UUID is a different player"
    );
    let _ = stop.send(());
    server.await.unwrap().unwrap();
}

#[tokio::test]
async fn shared_chest_transfers_over_quic_reject_stale_requests_and_survive_reopen() {
    tokio::time::timeout(Duration::from_secs(20), async {
        let (endpoint, cert) = make_server_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
        let address = endpoint.local_addr().unwrap();
        let manager = Arc::new(Mutex::new(VoxelWorld::new(12345)));
        let (stop, stopped) = oneshot::channel();
        let server = tokio::spawn(serve_endpoint(endpoint, manager.clone(), async {
            let _ = stopped.await;
        }));
        let (_e1, c1) = connect_endpoint(address, cert.clone()).await.unwrap();
        let (mut s1, mut r1) = c1.open_bi().await.unwrap();
        login(&mut s1, &mut r1, "ChestAlice").await;
        snapshot(&mut r1, 0).await;
        // Collect an existing world item, then approach the generated chest.
        for seq in 1..=30 {
            write_message(
                &mut s1,
                &ClientMessage::Input3d {
                    seq,
                    input: PlayerInput {
                        forward: if seq <= 12 { 1.0 } else { 0.0 },
                        strafe: if seq <= 12 { 1.0 } else { -1.0 },
                        pickup: true,
                        ..Default::default()
                    },
                },
            )
            .await
            .unwrap();
            snapshot(&mut r1, seq).await;
        }
        let alice = snapshot(&mut r1, 30).await;
        assert_eq!(alice.inventory.len(), 1);
        write_message(
            &mut s1,
            &ClientMessage::Inventory(InventoryCommand::Move {
                revision: 1,
                from: 0,
                to: 9,
                quantity: 1,
            }),
        )
        .await
        .unwrap();
        loop {
            let s = snapshot(&mut r1, 30).await;
            if s.inventory.revision == 2 {
                assert!(s.inventory.slots[0].is_none());
                assert!(s.inventory.slots[9].is_some());
                break;
            }
        }
        write_message(
            &mut s1,
            &ClientMessage::Inventory(InventoryCommand::Move {
                revision: 1,
                from: 9,
                to: 2,
                quantity: 1,
            }),
        )
        .await
        .unwrap();
        loop {
            let s = snapshot(&mut r1, 30).await;
            if !s.container_error.is_empty() {
                assert!(s.inventory.slots[9].is_some());
                assert!(s.inventory.slots[2].is_none());
                break;
            }
        }
        write_message(
            &mut s1,
            &ClientMessage::Inventory(InventoryCommand::QuickMove {
                revision: 2,
                slot: 9,
            }),
        )
        .await
        .unwrap();
        write_message(
            &mut s1,
            &ClientMessage::Inventory(InventoryCommand::Select { slot: 8 }),
        )
        .await
        .unwrap();
        loop {
            let s = snapshot(&mut r1, 30).await;
            if s.inventory.revision == 3 && s.inventory.selected == 8 {
                assert!(s.inventory.slots[9].is_none());
                assert!(s.inventory.slots[0].is_some());
                break;
            }
        }
        let (_e2, c2) = connect_endpoint(address, cert).await.unwrap();
        let (mut s2, mut r2) = c2.open_bi().await.unwrap();
        login(&mut s2, &mut r2, "ChestBob").await;
        snapshot(&mut r2, 0).await;
        let position = [26, 7, 30];
        for send in [&mut s1, &mut s2] {
            write_message(
                send,
                &ClientMessage::Container(ContainerCommand::Open { position }),
            )
            .await
            .unwrap();
        }
        for recv in [&mut r1, &mut r2] {
            loop {
                if snapshot(recv, 0).await.container.is_some() {
                    break;
                }
            }
        }
        let command = |revision, deposit| {
            ClientMessage::Container(ContainerCommand::Transfer {
                position,
                revision,
                inventory_revision: if deposit && revision == 0 {
                    3
                } else {
                    u64::from(deposit)
                },
                slot: 0,
                target: None,
                quantity: 1,
                deposit,
            })
        };
        write_message(&mut s1, &command(0, true)).await.unwrap();
        loop {
            let s = snapshot(&mut r2, 0).await;
            if s.container.as_ref().is_some_and(|c| c.revision == 1) {
                assert_eq!(s.container.unwrap().items[0].quantity, 1);
                break;
            }
        }
        // Two clients observed revision 1. Only the first withdrawal may commit.
        write_message(&mut s2, &command(1, false)).await.unwrap();
        loop {
            let s = snapshot(&mut r2, 0).await;
            if !s.inventory.is_empty() {
                assert_eq!(s.inventory.slots[0].as_ref().unwrap().quantity, 1);
                break;
            }
        }
        write_message(&mut s1, &command(1, false)).await.unwrap();
        loop {
            let s = snapshot(&mut r1, 0).await;
            if !s.container_error.is_empty() {
                assert!(s.inventory.is_empty());
                break;
            }
        }
        write_message(&mut s2, &command(2, true)).await.unwrap();
        loop {
            if snapshot(&mut r2, 0)
                .await
                .container
                .is_some_and(|c| c.revision == 3)
            {
                break;
            }
        }
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/chest-quic");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("world-{}.bin", std::process::id()));
        manager.lock().await.save(&path).unwrap();
        c1.close(0u32.into(), b"test complete");
        c2.close(0u32.into(), b"test complete");
        let _ = stop.send(());
        server.await.unwrap().unwrap();
        let mut restored = VoxelWorld::load(&path).unwrap();
        assert_eq!(restored.terrain.get(position), CHEST);
        let (tx, mut rx) = tokio::sync::mpsc::channel(64);
        restored.register_player(42, tx).unwrap();
        restored.container_command(42, ContainerCommand::Open { position });
        restored.tick();
        let mut loaded_contents = None;
        while let Ok(message) = rx.try_recv() {
            if let ServerMessage::World3d(s) = message {
                loaded_contents = s.container;
            }
        }
        let loaded = loaded_contents.expect("saved chest is usable after restart");
        assert_eq!(
            loaded.items,
            alice.inventory.iter().cloned().collect::<Vec<_>>()
        );
        assert_eq!(loaded.revision, 3);
    })
    .await
    .expect("chest QUIC integration timed out");
}
#[tokio::test]
async fn two_clients_receive_voxels_move_jump_chat_and_disconnect_over_verified_quic() {
    tokio::time::timeout(Duration::from_secs(20), async {
        let (endpoint, cert) = make_server_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
        let address = endpoint.local_addr().unwrap();
        let manager = Arc::new(Mutex::new(VoxelWorld::new(12345)));
        let (stop, stopped) = oneshot::channel();
        let server = tokio::spawn(serve_endpoint(endpoint, manager.clone(), async {
            let _ = stopped.await;
        }));
        let (_e1, c1) = connect_endpoint(address, cert.clone()).await.unwrap();
        let (mut s1, mut r1) = c1.open_bi().await.unwrap();
        let id1 = login(&mut s1, &mut r1, "Alice").await;
        let mut map = game_simulation::grid::Grid::unknown();
        let mut chunks = 0;
        let initial = loop {
            match read_message(&mut r1).await.unwrap() {
                ServerMessage::Chunk3d { origin, blocks } => {
                    assert!(map.apply_chunk(origin, &blocks));
                    chunks += 1;
                }
                ServerMessage::World3d(s) => break s,
                _ => {}
            }
        };
        assert_eq!(chunks, 32);
        assert!(!map.blocks.contains(&255));
        assert!(map.solid([28, 6, 32]));
        let (_e2, c2) = connect_endpoint(address, cert).await.unwrap();
        let (mut s2, mut r2) = c2.open_bi().await.unwrap();
        let id2 = login(&mut s2, &mut r2, "Bob").await;
        let bob = snapshot(&mut r2, 0).await;
        assert!(bob.actors.iter().any(|a| a.id == id1));
        assert_ne!(id1, id2);
        write_message(
            &mut s1,
            &ClientMessage::Input3d {
                seq: 1,
                input: PlayerInput {
                    forward: 1.0,
                    jump: true,
                    ..Default::default()
                },
            },
        )
        .await
        .unwrap();
        let moved = snapshot(&mut r1, 1).await;
        assert!(moved.body.position[2] < initial.body.position[2]);
        assert!(moved.body.position[1] > initial.body.position[1] + 0.1);
        assert!(moved.actors.iter().any(|a| a.id == id2));
        let observed = loop {
            let s = snapshot(&mut r2, 0).await;
            if s.actors
                .iter()
                .any(|a| a.id == id1 && a.position[1] > initial.body.position[1] + 0.1)
            {
                break s;
            }
        };
        assert_eq!(
            observed.seq, 0,
            "ack is per player, never the simulation tick"
        );
        write_message(
            &mut s1,
            &ClientMessage::Chat3d {
                text: "hello in 3D".into(),
            },
        )
        .await
        .unwrap();
        loop {
            let s = snapshot(&mut r2, 0).await;
            if s.log.iter().any(|e| e.text.contains("hello in 3D")) {
                break;
            }
        }
        write_message(&mut s2, &ClientMessage::Logout)
            .await
            .unwrap();
        loop {
            let s = snapshot(&mut r1, 1).await;
            if !s.actors.iter().any(|a| a.id == id2) {
                break;
            }
        }
        write_message(&mut s1, &ClientMessage::Logout)
            .await
            .unwrap();
        let _ = stop.send(());
        server.await.unwrap().unwrap();
    })
    .await
    .expect("3D integration test timed out");
}
#[tokio::test]
async fn quic_transport_delivers_3d_snapshots_and_closes_remote_session() {
    let (endpoint, cert) = make_server_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let address = endpoint.local_addr().unwrap();
    let manager = Arc::new(Mutex::new(VoxelWorld::new(12345)));
    let (stop, stopped) = oneshot::channel();
    let server = tokio::spawn(serve_endpoint(endpoint, manager.clone(), async {
        let _ = stopped.await;
    }));
    let identity = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target/network-identities")
        .join(format!("{}.uuid", uuid::Uuid::new_v4()));
    let mut session =
        QuicSession::connect_identity(address, "adapter".into(), cert, identity).unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match session.poll() {
                Some(NetworkEvent::Message(ServerMessage::World3d(_))) => break,
                Some(NetworkEvent::Disconnected(reason)) => panic!("{reason}"),
                _ => tokio::time::sleep(Duration::from_millis(5)).await,
            }
        }
    })
    .await
    .unwrap();
    drop(session);
    tokio::time::timeout(Duration::from_secs(3), async {
        while manager.lock().await.player_count() != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("remote session leaked");
    let _ = stop.send(());
    server.await.unwrap().unwrap();
}
#[tokio::test]
async fn old_protocol_cannot_join_the_3d_world() {
    let (endpoint, cert) = make_server_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let address = endpoint.local_addr().unwrap();
    let manager = Arc::new(Mutex::new(VoxelWorld::new(1)));
    let (stop, stopped) = oneshot::channel();
    let server = tokio::spawn(serve_endpoint(endpoint, manager, async {
        let _ = stopped.await;
    }));
    let (_endpoint, connection) = connect_endpoint(address, cert).await.unwrap();
    let (mut send, mut recv) = connection.open_bi().await.unwrap();
    write_message(
        &mut send,
        &ClientMessage::Login {
            version: PROTOCOL_VERSION - 1,
            player_name: "old".into(),
            player_uuid: [1; 16],
        },
    )
    .await
    .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(3), read_message(&mut recv))
            .await
            .unwrap()
            .is_err()
    );
    let _ = stop.send(());
    server.await.unwrap().unwrap();
}

#[tokio::test]
async fn wrong_certificate_is_rejected_and_missing_file_surfaces_asynchronously() {
    let (endpoint, _) = make_server_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let address = endpoint.local_addr().unwrap();
    let (_other, wrong_cert) = make_server_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let manager = Arc::new(Mutex::new(VoxelWorld::new(1)));
    let (stop, stopped) = oneshot::channel();
    let server = tokio::spawn(serve_endpoint(endpoint, manager.clone(), async {
        let _ = stopped.await;
    }));
    assert!(connect_endpoint(address, wrong_cert).await.is_err());
    assert_eq!(manager.lock().await.player_count(), 0);
    let mut transport = QuicSession::connect_file_identity(
        address,
        "missing".into(),
        format!("target/nonexistent-certificate-{}.der", std::process::id()),
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../target/network-identities")
            .join(format!("{}.uuid", uuid::Uuid::new_v4())),
    )
    .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(NetworkEvent::Disconnected(reason)) = transport.poll() {
                assert!(reason.contains("read server certificate"));
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    drop(transport);
    let _ = stop.send(());
    server.await.unwrap().unwrap();
}
