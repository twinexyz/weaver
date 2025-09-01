//! DA (Data Availability) publisher functionality

use std::time::Duration;

use reth_tracing::tracing::info;
use sqlx::PgPool;
use twine_aggregator_common::config::AppCfg;

/// Start the DA publisher
pub(crate) async fn start_da_publisher(
    _config: &AppCfg,
    _db_pool: PgPool,
) -> eyre::Result<tokio::task::JoinHandle<()>> {
    // Spawn the publisher loop and return its handle
    let handle = tokio::spawn(async move {
        info!("Starting DA publisher...");
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });

    Ok(handle)
}
