use server::{network, storage, world};

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::storage::save::{load_world_from, save_world_to};
use crate::world::sub_world::SubWorldEvent;
use crate::world::world_manager::WorldManager;

const DEFAULT_SEED: u64 = 12345;

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

    let save_path = PathBuf::from(
        std::env::var("CDDA_SAVE_PATH").unwrap_or_else(|_| "./save/world.bin".to_string()),
    );

    // Preserve malformed saves: do not silently replace them with a new world.
    let mut wm = match load_world_from(&save_path) {
        Ok(saved) => {
            info!(
                path = %save_path.display(),
                world_seed = saved.world_seed,
                sub_world_count = saved.sub_worlds.len(),
                "Loaded world from save"
            );
            WorldManager::from_saved(saved)?
        }
        Err(e) => {
            if save_path.exists() {
                return Err(e.context("existing save could not be restored"));
            }
            info!(
                path = %save_path.display(),
                reason = %e,
                world_seed = DEFAULT_SEED,
                "No save loaded, starting fresh"
            );
            WorldManager::new(DEFAULT_SEED)
        }
    };

    let world_seed = wm.world_seed;
    let event_rx = wm.take_event_rx().expect("take_event_rx called once");
    let manager = Arc::new(Mutex::new(wm));

    // Spawn the cross-world transfer event processor.
    {
        let manager = manager.clone();
        tokio::spawn(async move {
            run_event_processor(manager, event_rx).await;
        });
    }

    let addr = std::env::var("CDDA_BIND")
        .unwrap_or_else(|_| "127.0.0.1:9876".into())
        .parse()?;
    info!(%addr, world_seed, "Starting server");

    let run_seconds = std::env::var("CDDA_RUN_SECONDS")
        .ok()
        .map(|s| s.parse::<u64>())
        .transpose()?;
    let shutdown = async {
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                if let Err(e) = result { error!(error = %e, "Failed to install ctrl_c handler"); }
            }
            _ = async {
                match run_seconds {
                    Some(seconds) => tokio::time::sleep(std::time::Duration::from_secs(seconds)).await,
                    None => std::future::pending::<()>().await,
                }
            } => {}
        }
        info!("Shutdown requested");
    };

    network::run_server(addr, manager.clone(), shutdown).await?;

    // Snapshot every active sub-world and persist before exit.
    info!("Saving world before exit");
    let saved = manager.lock().await.snapshot_all().await;
    match save_world_to(&saved, &save_path) {
        Ok(()) => info!(
            path = %save_path.display(),
            sub_world_count = saved.sub_worlds.len(),
            "World saved"
        ),
        Err(e) => error!(error = %e, path = %save_path.display(), "Failed to save world"),
    }

    Ok(())
}

async fn run_event_processor(
    manager: Arc<Mutex<WorldManager>>,
    mut event_rx: tokio::sync::mpsc::UnboundedReceiver<SubWorldEvent>,
) {
    while let Some(event) = event_rx.recv().await {
        match event {
            SubWorldEvent::TransferPlayer {
                player_id,
                from_sw,
                to_sw,
                pos,
                tx,
                previous,
            } => {
                info!(
                    player_id,
                    ?from_sw,
                    ?to_sw,
                    ?pos,
                    "Processing TransferPlayer event"
                );
                manager
                    .lock()
                    .await
                    .transfer_player(player_id, to_sw, pos, tx, Some(previous))
                    .await;
            }
        }
    }
    warn!("Event processor channel closed");
}
