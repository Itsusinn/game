mod network;
mod storage;
mod world;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "server=debug,tower=info".into()),
        )
        .with_target(true)
        .with_line_number(true)
        .init();

    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("install rustls crypto provider");

    let addr = "0.0.0.0:9876".parse()?;
    tracing::info!(%addr, world_seed = 12345, "Starting server");
    network::run_server(addr, 12345).await?;
    Ok(())
}
