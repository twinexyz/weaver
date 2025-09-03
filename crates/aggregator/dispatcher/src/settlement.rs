//! On Chain settlement pipeline

use std::sync::Arc;
use std::time::Duration;

use eyre::Result;
use reth_tracing::tracing::{info, warn};
use sqlx::PgPool;
use tokio::time;
use twine_aggregator_common::SettleBatch;
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

    loop {
        tick.tick().await;

        // Get the current checkpoint from the database on each iteration
        let cp = operations::get_last_processed_on_chain_batch(&pool, &chain).await?;
        let next = cp.saturating_add(1);

        if client.is_finalized(next).await? {
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

        match operations::get_settlement_batch_by_id(&pool, next).await? {
            Some(batch) => {
                let batch_id = batch.batch_number;
                if let Err(e) = client.settle(&batch).await {
                    warn!(batch = next, "Settlement failed: {e:?}");
                    continue;
                }
                update_on_chain_progress(
                    &pool,
                    batch_id,
                    &chain,
                    OnChainStatus::SendSuccessful,
                    Some("txn_hash"),
                    None,
                    None,
                )
                .await?;
                info!(batch = next, "Settlement completed");
            }
            None => { /* producer hasn't inserted BatchData yet */ }
        }
    }
}
