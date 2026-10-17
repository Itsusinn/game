use client::network_client::{
    connect_endpoint, read_message, write_message, NetworkEvent, QuicSession, Transport,
};
use protocol::*;
use server::{
    network::{make_server_endpoint, serve_endpoint},
    world::world_manager::WorldManager,
};
use std::{sync::Arc, time::Duration};
use tokio::sync::{oneshot, Mutex};

async fn snapshot(recv: &mut quinn::RecvStream, seq: u32) -> ServerMessage {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let message = read_message(recv).await.unwrap();
            if matches!(&message, ServerMessage::WorldState { seq: ack, .. } if *ack == seq) {
                return message;
            }
        }
    })
    .await
    .expect("snapshot timed out")
}

#[tokio::test]
async fn two_clients_login_move_acknowledge_and_disconnect_over_verified_quic() {
    tokio::time::timeout(Duration::from_secs(20), async {
        let (endpoint, cert) = make_server_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
        let address = endpoint.local_addr().unwrap();
        let manager = Arc::new(Mutex::new(WorldManager::new(12345)));
        let (stop, stopped) = oneshot::channel();
        let server = tokio::spawn(serve_endpoint(endpoint, manager.clone(), async {
            let _ = stopped.await;
        }));
        let (_endpoint1, connection1) = connect_endpoint(address, cert.clone()).await.unwrap();
        let (mut send1, mut recv1) = connection1.open_bi().await.unwrap();
        write_message(
            &mut send1,
            &ClientMessage::Login {
                version: PROTOCOL_VERSION,
                player_name: "Alice".into(),
            },
        )
        .await
        .unwrap();
        let id1 = match read_message(&mut recv1).await.unwrap() {
            ServerMessage::LoginAccepted { player_id, .. } => player_id,
            other => panic!("expected login, got {other:?}"),
        };
        let first = snapshot(&mut recv1, 0).await;
        let (_endpoint2, connection2) = connect_endpoint(address, cert).await.unwrap();
        let (mut send2, mut recv2) = connection2.open_bi().await.unwrap();
        write_message(
            &mut send2,
            &ClientMessage::Login {
                version: PROTOCOL_VERSION,
                player_name: "Bob".into(),
            },
        )
        .await
        .unwrap();
        let id2 = match read_message(&mut recv2).await.unwrap() {
            ServerMessage::LoginAccepted { player_id, .. } => player_id,
            other => panic!("expected login, got {other:?}"),
        };
        assert_ne!(id1, id2);
        let second = snapshot(&mut recv2, 0).await;
        if let ServerMessage::WorldState {
            player_pos,
            entities,
            ..
        } = second
        {
            assert!(entities
                .iter()
                .any(|e| e.id == id2 && e.pos == player_pos && e.is_player));
            assert!(
                entities.iter().any(|e| e.id == id1 && e.is_player),
                "second player sees first player"
            );
        }
        let (origin, tile, action) = match first {
            ServerMessage::WorldState {
                player_pos,
                visible_tiles,
                entities,
                ..
            } => {
                let steps = [
                    (1, 0, ActionType::MoveRight),
                    (-1, 0, ActionType::MoveLeft),
                    (0, 1, ActionType::MoveDown),
                    (0, -1, ActionType::MoveUp),
                ];
                let (tile, action) = steps
                    .into_iter()
                    .find_map(|(dx, dy, action)| {
                        let target = Coord::new(player_pos.x + dx, player_pos.y + dy);
                        visible_tiles
                            .iter()
                            .find(|t| {
                                t.pos == target
                                    && t.tile_type == 0
                                    && !entities.iter().any(|e| e.pos == target)
                            })
                            .map(|_| (target, action))
                    })
                    .expect("walkable neighbour");
                (player_pos, tile, action)
            }
            _ => unreachable!(),
        };
        // Bob may occupy the chosen destination. His disconnection must remove his entity.
        write_message(&mut send2, &ClientMessage::Logout)
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            while manager.lock().await.player_sub_world(id2).is_some() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        write_message(
            &mut send1,
            &ClientMessage::PlayerAction {
                seq: 1,
                action,
                target: None,
            },
        )
        .await
        .unwrap();
        if let ServerMessage::WorldState {
            player_pos,
            entities,
            ..
        } = snapshot(&mut recv1, 1).await
        {
            assert_ne!(player_pos, origin);
            assert_eq!(player_pos, tile);
            assert!(entities.iter().any(|e| e.id == id1 && e.pos == player_pos));
            assert!(!entities.iter().any(|e| e.id == id2));
        }
        write_message(
            &mut send1,
            &ClientMessage::PlayerAction {
                seq: 2,
                action: ActionType::Chat {
                    text: "hello".into(),
                },
                target: None,
            },
        )
        .await
        .unwrap();
        if let ServerMessage::WorldState { message_log, .. } = snapshot(&mut recv1, 2).await {
            assert!(message_log.iter().any(|m| m.text.contains("hello")));
        }
        write_message(&mut send1, &ClientMessage::Logout)
            .await
            .unwrap();
        let _ = stop.send(());
        server.await.unwrap().unwrap();
    })
    .await
    .expect("end-to-end test timed out");
}

#[tokio::test]
async fn desktop_transport_delivers_snapshots_and_can_shutdown_while_reading() {
    let (endpoint, cert) = make_server_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let address = endpoint.local_addr().unwrap();
    let (stop, stopped) = oneshot::channel();
    let manager = Arc::new(Mutex::new(WorldManager::new(12345)));
    let server = tokio::spawn(serve_endpoint(endpoint, manager.clone(), async {
        let _ = stopped.await;
    }));
    let mut session = QuicSession::connect(address, "adapter-test".into(), cert).unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match session.poll() {
                Some(NetworkEvent::Message(ServerMessage::WorldState { .. })) => break,
                Some(NetworkEvent::Disconnected(reason)) => panic!("{reason}"),
                _ => tokio::time::sleep(Duration::from_millis(10)).await,
            }
        }
    })
    .await
    .unwrap();
    drop(session);
    tokio::time::timeout(Duration::from_secs(3), async {
        while manager.lock().await.player_sub_world(1).is_some() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("desktop adapter must close the remote session promptly");
    let _ = stop.send(());
    server.await.unwrap().unwrap();
}
