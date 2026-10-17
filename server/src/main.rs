use anyhow::Result;
use server::{network, voxel::VoxelWorld};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "server=info".into()),
        )
        .init();
    let save_path = PathBuf::from(
        std::env::var("CDDA_SAVE_PATH").unwrap_or_else(|_| "save/world3d.bin".into()),
    );
    // A failed load is fatal. Never overwrite legacy or malformed saves.
    let world = if save_path.exists() {
        VoxelWorld::load(&save_path)?
    } else {
        VoxelWorld::new(12345)
    };
    let manager = Arc::new(Mutex::new(world));
    let addr = std::env::var("CDDA_BIND")
        .unwrap_or_else(|_| "127.0.0.1:9876".into())
        .parse()?;
    let seconds = std::env::var("CDDA_RUN_SECONDS")
        .ok()
        .map(|s| s.parse::<u64>())
        .transpose()?;
    let shutdown = async {
        tokio::select! {
            _=tokio::signal::ctrl_c()=>{},
            _=async {if let Some(s)=seconds {tokio::time::sleep(std::time::Duration::from_secs(s)).await;} else {std::future::pending::<()>().await;}}=>{},
        }
    };
    network::run_server(addr, manager.clone(), shutdown).await?;
    manager.lock().await.save(&save_path)?;
    tracing::info!(path=%save_path.display(),"3D world saved");
    Ok(())
}
