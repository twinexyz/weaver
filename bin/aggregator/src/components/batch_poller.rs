//! Batch poller functionality

use std::time::Duration;

use reth_tracing::tracing::{error, info};
use tokio::sync::mpsc;
use twine_aggregator_common::config::AppCfg;
use twine_aggregator_database::operations::{get_last_polled_batch, insert_batch};
use twine_l2_batch_poller::poll_batches;
use twine_types::settle::CommitBatch;

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

    let (batch_sender, mut batch_receiver) = mpsc::channel(100);

    let db_pool_clone = db_pool;
    let handle = tokio::spawn(async move {
        let _l2_poller = poll_batches(
            &twine_rpc,
            start_height,
            batch_sender,
            poll_interval,
            move |bm| {
                let db_pool = db_pool_clone.clone();
                tokio::spawn(async move {
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
                            }
                        }
                    }
                });

                CommitBatch {
                    batch_number: bm.batch_number,
                    batch_hash: bm.batch_hash.map(|h| h.0).unwrap_or_default(),
                }
            },
        )
        .await;
    });

    while let Some(x) = batch_receiver.recv().await {
        let _ = x;
    }

    Ok(handle)
}
