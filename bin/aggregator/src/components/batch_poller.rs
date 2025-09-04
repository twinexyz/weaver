//! Batch poller functionality

use std::time::Duration;

use reth_tracing::tracing::{debug, error, info};
use twine_aggregator_common::config::AppCfg;
use twine_aggregator_database::operations::{get_last_polled_batch, insert_batch};
use twine_l2_batch_poller::poll_batches_async;

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

    let db_pool_clone = db_pool;
    let handle = tokio::spawn(async move {
        info!("Batch poller task started");
        let _l2_poller = poll_batches_async(&twine_rpc, start_batch, poll_interval, move |bm| {
            let db_pool = db_pool_clone.clone();
            async move {
                info!("Observed twine batch: {}", bm.batch_number);

                // Record metrics for the observed batch
                twine_aggregator_metrics::record_twine_batch_observed(
                    &twine_chain_id.to_string(),
                    bm.batch_number,
                );

                if let Some(hash) = bm.batch_hash {
                    match insert_batch(&db_pool, bm.batch_number, hash.0).await {
                        Ok(()) => {
                            debug!(
                                "Successfully inserted batch {} into database",
                                bm.batch_number
                            );
                        }
                        Err(e) => {
                            error!(
                                "Failed to insert batch {} into database: {:?}",
                                bm.batch_number, e
                            );
                            return Err(eyre::eyre!("Failed to insert batch: {:?}", e));
                        }
                    }
                }
                Ok(())
            }
        })
        .await;
    });

    Ok(handle)
}
