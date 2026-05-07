use anyhow::{Context, Result};
use quinn::{Connection, Endpoint, RecvStream, SendStream, ServerConfig};
use rcgen::generate_simple_self_signed;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use protocol::{ClientMessage, Coord, ServerMessage};

use crate::world::world_manager::WorldManager;

pub fn make_server_endpoint(addr: std::net::SocketAddr) -> Result<Endpoint> {
    let certified_key = generate_simple_self_signed(vec!["localhost".to_string()])
        .context("generate self-signed cert")?;

    let cert_der = certified_key.cert.der().to_vec();
    let key_der = certified_key.key_pair.serialize_der();

    let tls_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(cert_der)],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_der)),
        )
        .context("build rustls server config")?;

    let quic_config = quinn::crypto::rustls::QuicServerConfig::try_from(tls_config)
        .context("convert to quic server config")?;

    let server_config = ServerConfig::with_crypto(Arc::new(quic_config));
    let endpoint = Endpoint::server(server_config, addr)
        .context("create quinn endpoint")?;

    Ok(endpoint)
}

pub async fn run_server(addr: std::net::SocketAddr, world_seed: u64) -> Result<()> {
    let endpoint = make_server_endpoint(addr)?;
    let wm = Arc::new(Mutex::new(WorldManager::new(world_seed)));

    println!("Server listening on udp://{} (seed: {})", addr, world_seed);

    loop {
        match endpoint.accept().await {
            Some(incoming) => {
                let wm = wm.clone();
                tokio::spawn(async move {
                    match incoming.await {
                        Ok(connection) => {
                            let addr = connection.remote_address();
                            println!("new connection: {:?}", addr);
                            if let Err(e) = handle_connection(connection, wm).await {
                                eprintln!("connection {:?} error: {}", addr, e);
                            }
                        }
                        Err(e) => {
                            eprintln!("incoming accept error: {e}");
                        }
                    }
                });
            }
            None => {
                println!("endpoint closed");
                break;
            }
        }
    }

    Ok(())
}

async fn handle_connection(
    connection: Connection,
    wm: Arc<Mutex<WorldManager>>,
) -> Result<()> {
    let (send, recv) = connection.accept_bi().await?;
    let (send, mut recv) = (send, recv);

    // Read login message
    let frame = read_frame(&mut recv).await?;
    let msg: ClientMessage = rmp_serde::from_slice(&frame)
        .context("deserialize login")?;

    let (pid, player_name, player_rx) = match msg {
        ClientMessage::Login {
            version: _,
            player_name,
        } => {
            let mut wm = wm.lock().await;
            let pid = wm.allocate_player_id();
            let (player_tx, player_rx) = mpsc::channel::<ServerMessage>(64);
            let spawn_pos = Coord::new(256, 256);
            wm.register_player(pid, spawn_pos, player_tx).await;
            println!("player {} ({}) logged in", pid, player_name);
            (pid, player_name, player_rx)
        }
        _ => anyhow::bail!("expected login message"),
    };

    // Spawn forwarder: SubWorld → client
    let send_arc = Arc::new(Mutex::new(send));
    let send_clone = send_arc.clone();
    tokio::spawn(async move {
        let mut rx = player_rx;
        while let Some(msg) = rx.recv().await {
            let data = match rmp_serde::to_vec(&msg) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("serialize server message: {e}");
                    break;
                }
            };
            let mut s = send_clone.lock().await;
            if write_frame(&mut *s, &data).await.is_err() {
                break;
            }
        }
    });

    let player_id = pid;
    let player_name = player_name;

    // Main loop: handle client messages
    loop {
        let frame = match read_frame(&mut recv).await {
            Ok(f) => f,
            Err(_) => break,
        };

        let msg: ClientMessage = match rmp_serde::from_slice(&frame) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("deserialize client message: {e}");
                continue;
            }
        };

        match msg {
            ClientMessage::PlayerAction {
                action, target, ..
            } => {
                let mut wm = wm.lock().await;
                wm.handle_player_action(player_id, action, target).await;
            }
            ClientMessage::Ping { seq } => {
                let pong = ServerMessage::Pong { seq };
                let data = rmp_serde::to_vec(&pong)?;
                let mut send = send_arc.lock().await;
                write_frame(&mut *send, &data).await?;
            }
            ClientMessage::Logout => break,
            _ => {}
        }
    }

    // Cleanup
    let mut wm = wm.lock().await;
    wm.unregister_player(player_id).await;
    println!("player {} ({}) disconnected", player_id, player_name);

    connection.close(0u32.into(), b"bye");
    Ok(())
}

async fn read_frame(recv: &mut RecvStream) -> Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf).await
        .context("read frame length")?;
    let len = u32::from_be_bytes(len_buf) as usize;

    if len > 1_048_576 {
        anyhow::bail!("frame too large: {len}");
    }

    let mut buf = vec![0u8; len];
    recv.read_exact(&mut buf).await
        .context("read frame body")?;
    Ok(buf)
}

async fn write_frame(send: &mut SendStream, data: &[u8]) -> Result<()> {
    let len = data.len() as u32;
    send.write_all(&len.to_be_bytes()).await
        .context("write frame length")?;
    send.write_all(data).await
        .context("write frame body")?;
    Ok(())
}
