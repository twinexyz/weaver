//! Batch poller functionality

use std::time::Duration;

use reth_tracing::tracing::{error, info};
use twine_aggregator_common::config::AppCfg;
use twine_aggregator_database::operations::{get_last_polled_batch, insert_batch};
use twine_aggregator_metrics::record_twine_batch_observed;
use twine_l2_batch_poller::poll_batches_async;

/// Start the L2 batch poller and return a handle for graceful shutdown
pub(crate) async fn start_batch_poller(
    config: &AppCfg,
    db_pool: sqlx::PgPool,
) -> eyre::Result<tokio::task::JoinHandle<()>> {
    let twine_rpc = config.twine.rpcs[0].clone();
    let start_from = config.twine.start_batch;
    let last_polled = get_last_polled_batch(&db_pool).await?;
    let start_height = start_from.max(last_polled.saturating_sub(1));
    let poll_interval = Duration::from_secs(config.twine.poll_interval);

    let db_pool_clone = db_pool;
    let handle = tokio::spawn(async move {
        let _l2_poller = poll_batches_async(&twine_rpc, start_height, poll_interval, move |bm| {
            let db_pool = db_pool_clone.clone();
            async move {
                // Record metrics for the observed batch
                record_twine_batch_observed("twine", bm.batch_number);

                if let Some(hash) = bm.batch_hash {
                    match insert_batch(&db_pool, bm.batch_number, hash.0).await {
                        Ok(()) => {
                            info!(
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
