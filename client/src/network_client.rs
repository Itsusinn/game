use anyhow::{Context, Result};
use quinn::{Connection, Endpoint, RecvStream, SendStream, ClientConfig};
use rustls::pki_types::ServerName;
use std::sync::Arc;
use std::net::SocketAddr;

use protocol::{ClientMessage, ServerMessage};

pub struct NetworkClient {
    endpoint: Endpoint,
    connection: Connection,
    send: SendStream,
    recv: RecvStream,
}

impl NetworkClient {
    pub async fn connect(server_addr: SocketAddr) -> Result<Self> {
        let provider = rustls::crypto::aws_lc_rs::default_provider();
        let tls_config = rustls::ClientConfig::builder_with_provider(provider.into())
            .with_protocol_versions(&[&rustls::version::TLS13])
            .context("set TLS 1.3")?
            .dangerous()
            .with_custom_certificate_verifier(skip_verifier())
            .with_no_client_auth();

        let quic_config = quinn::crypto::rustls::QuicClientConfig::try_from(tls_config)
            .context("convert to quic client config")?;

        let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)
            .context("create client endpoint")?;
        endpoint.set_default_client_config(ClientConfig::new(Arc::new(quic_config)));

        let connection = endpoint
            .connect(server_addr, "localhost")
            .context("connect to server")?
            .await
            .context("await connection")?;

        let (send, recv) = connection.open_bi().await.context("open bi stream")?;

        Ok(Self {
            endpoint,
            connection,
            send,
            recv,
        })
    }

    pub async fn send_message(&mut self, msg: &ClientMessage) -> Result<()> {
        let data = rmp_serde::to_vec(msg).context("serialize message")?;
        let len = data.len() as u32;
        self.send.write_all(&len.to_be_bytes()).await?;
        self.send.write_all(&data).await?;
        Ok(())
    }

    pub async fn recv_message(&mut self) -> Result<ServerMessage> {
        let mut len_buf = [0u8; 4];
        self.recv.read_exact(&mut len_buf).await?;
        let msg_len = u32::from_be_bytes(len_buf) as usize;
        if msg_len > 1_048_576 {
            anyhow::bail!("frame too large: {msg_len}");
        }
        let mut buf = vec![0u8; msg_len];
        self.recv.read_exact(&mut buf).await?;
        let msg: ServerMessage = rmp_serde::from_slice(&buf)?;
        Ok(msg)
    }

    pub fn close(&self) {
        self.connection.close(0u32.into(), b"bye");
    }
}

fn skip_verifier() -> Arc<SkipServerVerification> {
    Arc::new(SkipServerVerification)
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
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
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
