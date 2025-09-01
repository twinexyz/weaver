//! Dispatcher functionality

use std::time::Duration;

use reth_tracing::tracing::info;
use sqlx::PgPool;
use twine_aggregator_common::config::AppCfg;

/// Start the dispatcher
pub(crate) async fn start_dispatcher(
    _config: &AppCfg,
    _db_pool: PgPool,
) -> eyre::Result<tokio::task::JoinHandle<()>> {
    // Spawn the dispatcher loop and return its handle
    let handle = tokio::spawn(async move {
        info!("Starting dispatcher...");
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });

    Ok(handle)
}
