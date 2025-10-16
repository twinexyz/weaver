//! Egressa service startup logic

use reth_tracing::tracing::{info, warn};
#[cfg(unix)]
use tokio::signal::unix::{signal, SignalKind};
use twine_egressa::config::AppCfg;
use twine_egressa::database::client::DbClient;

pub(crate) async fn start_egressa(config: &AppCfg) -> eyre::Result<()> {
    info!("Starting egressa service with config: {:?}", config);

    let db_client = DbClient::new(
        config.database.url.clone().as_str(),
        config.database.indexer_database_url.clone().as_str(),
    )
    .await?;

    let mut handles: Vec<tokio::task::JoinHandle<()>> = Vec::new();

    let config_clone = config.clone();
    let handle = tokio::spawn(async move {
        let _ = twine_egressa::service::run_service(config_clone, db_client.clone()).await;
    });

    handles.push(handle);

    info!("Egressa service is now running...");

    // Wait for shutdown signal (Ctrl-C on all platforms; SIGTERM on Unix)
    #[cfg(unix)]
    {
        let mut sigterm = signal(SignalKind::terminate())?;
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Received Ctrl-C. Initiating graceful shutdown...");
            }
            _ = sigterm.recv() => {
                info!("Received SIGTERM. Initiating graceful shutdown...");
            }
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await?;
        info!("Received Ctrl-C. Initiating graceful shutdown...");
    }

    // Abort background tasks and wait for completion
    info!("Aborting background tasks...");
    for h in &handles {
        h.abort();
    }

    info!("Waiting for tasks to complete...");
    for (idx, h) in handles.into_iter().enumerate() {
        if let Err(e) = h.await {
            if !e.is_cancelled() {
                warn!("Task #{idx} ended unexpectedly: {:?}", e);
            }
        }
    }

    info!("Egressa service stopped. Clean shutdown complete.");
    Ok(())
}
