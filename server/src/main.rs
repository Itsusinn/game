mod network;
mod world;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("install rustls crypto provider");

    let addr = "0.0.0.0:9876".parse()?;
    network::run_server(addr, 12345).await?;
    Ok(())
}
