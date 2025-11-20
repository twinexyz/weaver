//! Batch poller functionality

use std::time::Duration;

use reth_tracing::tracing::{debug, error, info};
use twine_aggregator_common::config::AppCfg;
use twine_aggregator_database::operations::{get_last_polled_batch, insert_batch};
use twine_l2_batch_poller::poll_batches_async;
use twine_types::VersionedBatchMeta;

/// Start the L2 batch poller and return a handle for graceful shutdown
pub(crate) async fn start_batch_poller(
    config: &AppCfg,
    db_pool: sqlx::PgPool,
) -> eyre::Result<tokio::task::JoinHandle<()>> {
    let twine_rpc = config.twine.rpc.clone();
    let twine_chain_id = config.twine.chain_id;
    let start_from = config.twine.start_batch;
    let last_polled = get_last_polled_batch(&db_pool).await?;
    let start_batch = start_from.max(last_polled.saturating_sub(1));
    let poll_interval = Duration::from_secs(config.twine.poll_interval);

    info!(
        "Batch poller configured: RPC={}, start_batch={}, poll_interval={}s",
        twine_rpc,
        start_batch,
        poll_interval.as_secs()
    );

    let handle = tokio::spawn(run_batch_poller(
        twine_rpc,
        twine_chain_id,
        start_batch,
        poll_interval,
        db_pool,
    ));
    Ok(handle)
}

async fn run_batch_poller(
    twine_rpc: String,
    twine_chain_id: u64,
    start_batch: u64,
    poll_interval: Duration,
    db_pool: sqlx::PgPool,
) {
    if let Err(e) = poll_batches_async(&twine_rpc, start_batch, poll_interval, {
        let handler_pool = db_pool.clone();
        move |bm| {
            let pool = handler_pool.clone();
            async move { handle_polled_batch(pool, twine_chain_id, bm).await }
        }
    })
    .await
    {
        error!("Batch poller terminated with error: {:?}", e);
    }
}

async fn handle_polled_batch(
    db_pool: sqlx::PgPool,
    twine_chain_id: u64,
    bm: VersionedBatchMeta,
) -> eyre::Result<()> {
    let batch_number = bm.batch_number();

    info!("Observed twine batch: {batch_number}");

    twine_aggregator_metrics::record_twine_batch_observed(
        &twine_chain_id.to_string(),
        batch_number,
    );

    if let Some(hash) = bm.batch_hash() {
        match insert_batch(&db_pool, batch_number, hash.0).await {
            Ok(()) => {
                debug!("Successfully inserted batch {batch_number} into database");
            }
            Err(e) => {
                error!(
                    "Failed to insert batch {batch_number} into database: {:?}",
                    e
                );
                return Err(eyre::eyre!("Failed to insert batch: {:?}", e));
            }
        }
    }

    Ok(())
}
