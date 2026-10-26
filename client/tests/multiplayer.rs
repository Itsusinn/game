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
    write_message(
        send,
        &ClientMessage::Login {
            version: PROTOCOL_VERSION,
            player_name: name.into(),
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
    let mut session = QuicSession::connect(address, "adapter".into(), cert).unwrap();
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
            version: 2,
            player_name: "old".into(),
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
    let mut transport = QuicSession::connect_file(
        address,
        "missing".into(),
        format!("target/nonexistent-certificate-{}.der", std::process::id()),
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
