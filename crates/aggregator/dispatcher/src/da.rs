//! DA Posting pipeline

use eyre::Result;
use reth_tracing::tracing::{info, warn};
use sqlx::PgPool;
use tokio::time::{self, Duration};
use twine_aggregator_common::{DALayer, TwineQuery};
use twine_aggregator_database::operations;

/// DA pipeline function that can be spawned as a task
pub async fn run_da_pipeline<TQ, DA>(pool: PgPool, twine: TQ, da: DA, poll_ms: u64) -> Result<()>
where
    TQ: TwineQuery + Send + Sync + 'static,
    DA: DALayer + Send + Sync + 'static, {
    let mut tick = time::interval(Duration::from_millis(poll_ms));
    let chain_id = da.chain_id().to_string();

    loop {
        tick.tick().await;

        // Get the current checkpoint from the database on each iteration
        let cp = operations::get_last_processed_da_batch(&pool, &chain_id).await?;
        let next = cp.saturating_add(1);

        // Ask Twine chain for the DA payload for this batch.
        match twine.da_payload(next).await {
            Ok(Some(bytes)) => {
                if let Err(e) = da.post(&bytes).await {
                    warn!(batch = next, "DA post failed: {e:?}");
                    continue; // retry on next tick
                }
                // Mark DA checkpoint advanced.
                // Start a transaction
                // Update da_status table as well as cursor
                operations::update_last_processed_da_batch(&pool, &chain_id, next).await?;
                info!(batch = next, "DA posted and advanced");
            }
            Ok(None) => {
                // Twine hasn't materialized the payload yet; keep polling.
            }
            Err(e) => {
                warn!(batch = next, "Twine query failed: {e:?}");
            }
        }
    }
}
