//! DA (Data Availability) verifier functionality

use std::time::Duration;

use reth_tracing::tracing::info;
use sqlx::PgPool;
use twine_aggregator_common::config::AppCfg;

/// Start the DA verifier
pub(crate) async fn start_da_verifier(
    _config: &AppCfg,
    _db_pool: PgPool,
) -> eyre::Result<tokio::task::JoinHandle<()>> {
    // Spawn the verifier loop and return its handle
    let handle = tokio::spawn(async move {
        info!("Starting DA verifier...");
        loop {
            tokio::time::sleep(Duration::from_secs(15)).await;
        }
    });

    Ok(handle)
}
