use anyhow::{Context, Result};
use quinn::Endpoint;
use rustls::pki_types::ServerName;
use std::sync::Arc;

use protocol::{ClientMessage, ServerMessage};

#[tokio::main]
async fn main() -> Result<()> {
    let server_addr: std::net::SocketAddr = "127.0.0.1:9876".parse()?;

    let provider = rustls::crypto::aws_lc_rs::default_provider();
    let tls_config = rustls::ClientConfig::builder_with_provider(provider.into())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .context("set TLS 1.3")?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
        .with_no_client_auth();

    let quic_config = quinn::crypto::rustls::QuicClientConfig::try_from(tls_config)
        .context("convert to quic client config")?;

    let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)
        .context("create client endpoint")?;
    endpoint.set_default_client_config(quinn::ClientConfig::new(Arc::new(quic_config)));

    let connection = endpoint
        .connect(server_addr, "localhost")
        .context("connect to server")?
        .await
        .context("await connection")?;

    println!("connected to server");

    let (mut send, mut recv) = connection
        .open_bi()
        .await
        .context("open bi stream")?;

    // Step 1: Login
    let login = ClientMessage::Login {
        version: 1,
        player_name: "test_client".into(),
    };
    let login_data = rmp_serde::to_vec(&login).context("serialize login")?;
    let len = login_data.len() as u32;
    send.write_all(&len.to_be_bytes()).await?;
    send.write_all(&login_data).await?;
    println!("sent login");

    // Step 2: Read WorldState from SubWorld
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf).await?;
    let msg_len = u32::from_be_bytes(len_buf) as usize;
    let mut buf = vec![0u8; msg_len];
    recv.read_exact(&mut buf).await?;
    let response: ServerMessage = rmp_serde::from_slice(&buf)?;
    match &response {
        ServerMessage::WorldState {
            player_pos,
            entities,
            hp,
            ..
        } => {
            println!(
                "received WorldState: pos=({},{}), entities={}, hp={}",
                player_pos.x,
                player_pos.y,
                entities.len(),
                hp
            );
        }
        other => {
            eprintln!("unexpected response after login: {:?}", other);
        }
    }

    // Step 3: Ping-Pong
    let ping = ClientMessage::Ping { seq: 1 };
    let ping_data = rmp_serde::to_vec(&ping)?;
    let len = ping_data.len() as u32;
    send.write_all(&len.to_be_bytes()).await?;
    send.write_all(&ping_data).await?;
    println!("sent ping seq=1");

    // Read frames until we get Pong (may get WorldState broadcasts from AI tick)
    for _ in 0..10 {
        let mut len_buf = [0u8; 4];
        recv.read_exact(&mut len_buf).await?;
        let msg_len = u32::from_be_bytes(len_buf) as usize;
        let mut buf = vec![0u8; msg_len];
        recv.read_exact(&mut buf).await?;
        let msg: ServerMessage = rmp_serde::from_slice(&buf)?;
        match msg {
            ServerMessage::Pong { seq } => {
                println!("Ping-Pong roundtrip successful! seq={seq}");
                break;
            }
            ServerMessage::WorldState { seq, .. } => {
                println!("(received WorldState seq={seq}, waiting for Pong...)");
            }
            other => {
                println!("(received {:?})", other);
            }
        }
    }

    // Step 4: Logout
    let logout = ClientMessage::Logout;
    let logout_data = rmp_serde::to_vec(&logout)?;
    let len = logout_data.len() as u32;
    send.write_all(&len.to_be_bytes()).await?;
    send.write_all(&logout_data).await?;

    connection.close(0u32.into(), b"bye");
    endpoint.wait_idle().await;
    Ok(())
}

#[derive(Debug)]
struct SkipServerVerification;

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer,
        _intermediates: &[rustls::pki_types::CertificateDer],
        _server_name: &ServerName,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<
        rustls::client::danger::ServerCertVerified,
        rustls::Error,
    > {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<
        rustls::client::danger::HandshakeSignatureValid,
        rustls::Error,
    > {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<
        rustls::client::danger::HandshakeSignatureValid,
        rustls::Error,
    > {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        use rustls::SignatureScheme;
        vec![
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ECDSA_NISTP521_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
        ]
    }
}
