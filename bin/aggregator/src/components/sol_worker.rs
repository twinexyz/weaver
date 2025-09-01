//! Solana worker functionality

use std::time::Duration;

use reth_tracing::tracing::info;
use sqlx::PgPool;
use twine_aggregator_common::config::AppCfg;

/// Start the Solana worker
pub(crate) async fn start_sol_worker(
    _config: &AppCfg,
    _db_pool: PgPool,
) -> eyre::Result<tokio::task::JoinHandle<()>> {
    // Spawn the worker loop and return its handle
    let handle = tokio::spawn(async move {
        info!("Starting Solana worker...");
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });

    Ok(handle)
}
