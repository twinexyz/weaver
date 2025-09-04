//! DA Posting pipeline

use eyre::Result;
use reth_tracing::tracing::{debug, info, warn};
use sqlx::types::JsonValue;
use sqlx::PgPool;
use tokio::time::{self, Duration};
use twine_aggregator_common::{DALayer, TwineQuery};
use twine_aggregator_database::types::DaPostingStatus;
use twine_aggregator_database::{operations, transactions};
use twine_aggregator_metrics::record_batch_dispatched;

/// Helper function to update da chain progress in a transaction
async fn update_da_progress(
    pool: &PgPool,
    batch_id: u64,
    da_id: &str,
    status: DaPostingStatus,
    verification_data: Option<&JsonValue>,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    transactions::upsert_da_status(
        &mut tx,
        batch_id,
        da_id,
        status,
        verification_data,
        None,
        None,
    )
    .await?;
    transactions::set_da_progress(&mut tx, da_id, batch_id).await?;
    tx.commit().await?;
    Ok(())
}

/// DA pipeline function that can be spawned as a task
pub async fn run_da_pipeline<TQ, DA>(pool: PgPool, twine: TQ, da: DA, poll_ms: u64) -> Result<()>
where
    TQ: TwineQuery + Send + Sync + 'static,
    DA: DALayer + Send + Sync + 'static, {
    let mut tick = time::interval(Duration::from_millis(poll_ms));
    let da_id = da.chain_id().to_string();
    info!(
        "Starting DA pipeline for chain {} with tick of {:?}",
        da_id,
        tick.period()
    );

    loop {
        tick.tick().await;

        // Get the current checkpoint from the database on each iteration
        let cp = operations::get_last_processed_da_batch(&pool, &da_id).await?;
        let next = cp.saturating_add(1);
        debug!("Processing DA batch: {} for chain {}", next, da_id);

        // Ask Twine chain for the DA payload for this batch.
        match twine.da_payload(next).await {
            Ok(Some(bytes)) => {
                debug!(batch = next, chain = da_id, "Posting DA payload for batch");
                match da.post(&bytes).await {
                    Ok(_) => {
                        info!(batch = next, chain = da_id, "DA post successful");
                        // mocks, this response should come from celestia
                        let val = Some(JsonValue::Null);
                        update_da_progress(
                            &pool,
                            next,
                            &da_id,
                            DaPostingStatus::Committed,
                            val.as_ref(),
                        )
                        .await?;
                        debug!(batch = next, chain = da_id, "DA status updated in database");
                        // Record successful DA post
                        record_batch_dispatched(&da_id, next);
                    }
                    Err(e) => {
                        warn!(batch = next, chain = da_id, error = ?e, "DA post failed");
                        continue; // retry on next tick
                    }
                }
            }
            Ok(None) => {
                // Twine hasn't materialized the payload yet; keep polling.
                debug!(
                    batch = next,
                    chain = da_id,
                    "Twine hasn't materialized DA payload yet"
                );
            }
            Err(e) => {
                warn!(batch = next, chain = da_id, error = ?e, "Twine query failed");
            }
        }
    }
}
