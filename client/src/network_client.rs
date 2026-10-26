//! Desktop QUIC adapter. A persistent reader owns the receive stream; sending
//! actions cannot cancel a partially consumed length-prefixed frame.
use anyhow::{Context, Result};
use protocol::{ClientMessage, ServerMessage, PROTOCOL_VERSION};
use quinn::{Connection, Endpoint, RecvStream, SendStream};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::sync::{mpsc, oneshot};

pub enum NetworkEvent {
    Message(ServerMessage),
    Disconnected(String),
}
pub trait Transport {
    fn send(&self, message: ClientMessage) -> Result<(), String>;
    fn poll(&mut self) -> Option<NetworkEvent>;
    /// Final application shutdown, after the UI event loop has already exited.
    fn shutdown(&mut self) {}
}

pub struct QuicSession {
    outgoing: mpsc::Sender<ClientMessage>,
    incoming: mpsc::Receiver<NetworkEvent>,
    shutdown: Option<oneshot::Sender<()>>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl QuicSession {
    pub fn connect(address: SocketAddr, name: String, certificate: Vec<u8>) -> Result<Self> {
        Self::connect_with(address, name, move || Ok(certificate))
    }
    pub fn connect_file(address: SocketAddr, name: String, path: String) -> Result<Self> {
        Self::connect_with(address, name, move || {
            std::fs::read(&path).with_context(|| format!("read server certificate: {path}"))
        })
    }
    fn connect_with(
        address: SocketAddr,
        name: String,
        certificate: impl FnOnce() -> Result<Vec<u8>> + Send + 'static,
    ) -> Result<Self> {
        let (outgoing, outgoing_rx) = mpsc::channel(64);
        let (incoming_tx, incoming) = mpsc::channel(256);
        let (shutdown, shutdown_rx) = oneshot::channel();
        let worker = std::thread::Builder::new()
            .name("game-network".into())
            .spawn(move || {
                let certificate = match certificate() {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        let _ = incoming_tx.try_send(NetworkEvent::Disconnected(format!("{e:#}")));
                        return;
                    }
                };
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(r) => r,
                    Err(e) => {
                        let _ = incoming_tx.try_send(NetworkEvent::Disconnected(e.to_string()));
                        return;
                    }
                };
                runtime.block_on(async move {
                    let result = run_session(
                        address,
                        name,
                        certificate,
                        outgoing_rx,
                        incoming_tx.clone(),
                        shutdown_rx,
                    )
                    .await;
                    let reason = match result {
                        Ok(()) => "Connection closed".into(),
                        Err(e) => format!("{e:#}"),
                    };
                    let _ = incoming_tx.send(NetworkEvent::Disconnected(reason)).await;
                });
            })?;
        Ok(Self {
            outgoing,
            incoming,
            shutdown: Some(shutdown),
            worker: Some(worker),
        })
    }
}
impl Transport for QuicSession {
    fn send(&self, message: ClientMessage) -> Result<(), String> {
        self.outgoing
            .try_send(message)
            .map_err(|e| format!("Unable to queue action: {e}"))
    }
    fn poll(&mut self) -> Option<NetworkEvent> {
        self.incoming.try_recv().ok()
    }
    fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        self.incoming.close();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
impl Drop for QuicSession {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        // Wake a worker waiting to deliver a final message. The worker owns its
        // runtime and finishes asynchronously; never join it on the UI thread.
        self.incoming.close();
        self.worker.take();
    }
}

pub async fn connect_endpoint(
    address: SocketAddr,
    certificate: Vec<u8>,
) -> Result<(Endpoint, Connection)> {
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(rustls::pki_types::CertificateDer::from(certificate))
        .context("load server certificate")?;
    let tls = rustls::ClientConfig::builder_with_provider(
        rustls::crypto::aws_lc_rs::default_provider().into(),
    )
    .with_protocol_versions(&[&rustls::version::TLS13])?
    .with_root_certificates(roots)
    .with_no_client_auth();
    let crypto = quinn::crypto::rustls::QuicClientConfig::try_from(tls)?;
    let bind = if address.is_ipv6() {
        "[::]:0"
    } else {
        "0.0.0.0:0"
    };
    let mut endpoint = Endpoint::client(bind.parse()?)?;
    endpoint.set_default_client_config(quinn::ClientConfig::new(Arc::new(crypto)));
    let connection = tokio::time::timeout(
        Duration::from_secs(5),
        endpoint.connect(address, "localhost")?,
    )
    .await??;
    Ok((endpoint, connection))
}
async fn run_session(
    address: SocketAddr,
    name: String,
    certificate: Vec<u8>,
    mut outgoing: mpsc::Receiver<ClientMessage>,
    incoming: mpsc::Sender<NetworkEvent>,
    mut shutdown: oneshot::Receiver<()>,
) -> Result<()> {
    let (endpoint, connection) = tokio::select! {
        _ = &mut shutdown => return Ok(()),
        result = connect_endpoint(address, certificate) => result?,
    };
    let io = async {
        let (mut send, mut recv) = connection.open_bi().await?;
        write_message(
            &mut send,
            &ClientMessage::Login {
                version: PROTOCOL_VERSION,
                player_name: name,
            },
        )
        .await?;
        let read = async {
            loop {
                incoming
                    .send(NetworkEvent::Message(read_message(&mut recv).await?))
                    .await
                    .context("UI receiver closed")?;
            }
            #[allow(unreachable_code)]
            Ok::<(), anyhow::Error>(())
        };
        let write = async {
            while let Some(message) = outgoing.recv().await {
                write_message(&mut send, &message).await?;
            }
            Ok::<(), anyhow::Error>(())
        };
        tokio::select! { result = read => result, result = write => result }
    };
    let result = tokio::select! {
        _ = &mut shutdown => Ok(()),
        result = io => result,
    };
    connection.close(0u32.into(), b"client shutdown");
    // Keep the runtime alive long enough to transmit CONNECTION_CLOSE.
    let _ = tokio::time::timeout(Duration::from_secs(1), endpoint.wait_idle()).await;
    result
}
pub async fn write_message(send: &mut SendStream, message: &ClientMessage) -> Result<()> {
    let data = rmp_serde::to_vec(message)?;
    anyhow::ensure!(data.len() <= 1_048_576, "message too large");
    send.write_all(&(data.len() as u32).to_be_bytes()).await?;
    send.write_all(&data).await?;
    Ok(())
}
pub async fn read_message(recv: &mut RecvStream) -> Result<ServerMessage> {
    let mut prefix = [0; 4];
    recv.read_exact(&mut prefix).await?;
    let size = u32::from_be_bytes(prefix) as usize;
    anyhow::ensure!(size <= 1_048_576, "frame too large: {size}");
    let mut data = vec![0; size];
    recv.read_exact(&mut data).await?;
    Ok(rmp_serde::from_slice(&data)?)
}
