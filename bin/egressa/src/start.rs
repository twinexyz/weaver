//! Egressa service startup logic

use log::{info, warn};
#[cfg(unix)]
use tokio::signal::unix::{signal, SignalKind};
use twine_egressa::config::AppCfg;

pub(crate) async fn start_egressa(config: &AppCfg) -> eyre::Result<()> {
    info!("Starting egressa service with config: {:?}", config);

    info!("Connecting to indexer database");
    let indexer_db_pool = sqlx::PgPool::connect(&config.database.indexer_database_url).await?;
    info!("Indexer database connection established");

    info!("Connecting to database");
    let db_pool = sqlx::PgPool::connect(&config.database.url).await?;
    info!("Database connection established");


    

    // Start your service components here
    let mut handles: Vec<tokio::task::JoinHandle<()>> = Vec::new();

    // Example: Start a background task
    let config_clone = config.clone();
    let handle = tokio::spawn(async move {
        twine_egressa::service::run_service(config_clone).await;
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
