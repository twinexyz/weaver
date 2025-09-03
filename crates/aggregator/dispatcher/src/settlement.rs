//! On Chain settlement pipeline

use std::sync::Arc;
use std::time::Duration;

use eyre::Result;
use reth_tracing::tracing::{debug, info, warn};
use sqlx::PgPool;
use tokio::time;
use twine_aggregator_common::{SettleBatch, TransactionStatus};
use twine_aggregator_database::types::OnChainStatus;
use twine_aggregator_database::{operations, transactions};

/// Helper function to update on-chain progress in a transaction
async fn update_on_chain_progress(
    pool: &PgPool,
    batch_id: u64,
    chain_id: &str,
    status: OnChainStatus,
    batch_posted_txn: Option<&str>,
    error_msg: Option<&str>,
    posted_at: Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    transactions::upsert_on_chain_status(
        &mut tx,
        batch_id,
        chain_id,
        status,
        batch_posted_txn,
        error_msg,
        posted_at,
    )
    .await?;
    transactions::set_on_chain_progress(&mut tx, chain_id, batch_id).await?;
    tx.commit().await?;
    Ok(())
}

/// Settlement pipeline function that can be spawned as a task
pub async fn run_settlement_pipeline(
    pool: PgPool,
    client: Arc<dyn SettleBatch + Send + Sync>,
    poll_ms: u64,
) -> Result<()> {
    let chain = client.chain_id().to_string();
    let mut tick = time::interval(Duration::from_millis(poll_ms));
    info!(
        "Starting settlement pipeline for chain {} with tick of {:?}",
        chain,
        tick.period()
    );

    loop {
        tick.tick().await;

        // Get the current checkpoint from the database on each iteration
        let cp = operations::get_last_processed_on_chain_batch(&pool, &chain).await?;
        let next = cp.saturating_add(1);
        debug!("Processing batch: {} for {}", next, chain);

        // Check if batch is already finalized on chain
        match client.is_finalized(next).await {
            Ok(true) => {
                debug!("Batch {} already finalized on chain {}", next, chain);
                update_on_chain_progress(
                    &pool,
                    next,
                    &chain,
                    OnChainStatus::SendSuccessful,
                    None,
                    None,
                    None,
                )
                .await?;
                continue;
            }
            Ok(false) => {
                debug!("Batch {} not yet finalized on chain {}", next, chain);
            }
            Err(e) => {
                warn!(
                    "Error checking if batch {} is finalized on chain {}: {:?}",
                    next, chain, e
                );
                continue;
            }
        }

        // Check if execution proof exists for this batch
        match operations::execution_proof_exists(&pool, next).await {
            Ok(exists) =>
                if !exists {
                    debug!(
                        "Execution proof for batch {} does not exist, skipping",
                        next
                    );
                    continue;
                },
            Err(e) => {
                warn!(
                    "Error checking if execution proof exists for batch {}: {:?}",
                    next, e
                );
                continue;
            }
        }

        match operations::get_settlement_batch_by_id(&pool, next).await {
            Ok(Some(batch)) => {
                let batch_id = batch.batch_number;
                debug!("Attempting to settle batch {} on chain {}", batch_id, chain);

                match client.settle(&batch).await {
                    Ok(TransactionStatus {
                        status,
                        txn_hash,
                        message,
                    }) => {
                        if status {
                            // Transaction was successful
                            update_on_chain_progress(
                                &pool,
                                batch_id,
                                &chain,
                                OnChainStatus::SendSuccessful,
                                Some(&txn_hash),
                                None,
                                None,
                            )
                            .await?;
                            info!(
                                batch = next,
                                chain = chain,
                                txn_hash = txn_hash,
                                "Settlement completed successfully"
                            );
                        } else {
                            // Transaction failed on-chain
                            update_on_chain_progress(
                                &pool,
                                batch_id,
                                &chain,
                                OnChainStatus::SendFailed,
                                Some(&txn_hash),
                                message.as_deref(),
                                None,
                            )
                            .await?;
                            warn!(
                                batch = next,
                                chain = chain,
                                txn_hash = txn_hash,
                                error = message,
                                "Settlement failed on-chain"
                            );
                        }
                    }
                    Err(e) => {
                        // Other error occurred
                        warn!(batch = next, chain = chain, error = ?e, "Settlement failed");
                        continue;
                    }
                }
            }
            Ok(None) => {
                debug!(
                    "Settlement batch not found for batch {}, proof may be missing",
                    next
                );
            }
            Err(e) => {
                warn!(batch = next, chain = chain, error = ?e, "Error retrieving settlement batch");
            }
        }
    }
}
