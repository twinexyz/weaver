use eyre::Result;
use reth_tracing::tracing::{error, info, warn};

use crate::chains::twine::provider::TwineProvider;
use crate::database::client::DbClient;
use crate::types::{WithdrawalEvent, WithdrawalEventType};

/// Dummy polling service for testing
#[derive(Debug)]
pub struct WithdrawalEventPoller;

impl WithdrawalEventPoller {
    /// Poll for pending transaction events from indexer database
    pub async fn poll_events(
        &self,
        db_client: DbClient,
        twine_provider: &TwineProvider,
    ) -> Result<Vec<WithdrawalEvent>> {
        info!("Polling for pending transaction events from database...");

        // Get the latest batch information to filter events
        let max_block_height = match Self::get_latest_batched_block_height(twine_provider).await {
            Ok(height) => {
                info!("Latest batched block height: {}", height);
                Some(height)
            }
            Err(e) => {
                error!("Failed to get latest batched block height: {}", e);
                warn!("Proceeding without block height filter - this may cause processing of unbatched blocks");
                None
            }
        };

        // Query the database for pending events with block height filter
        let events = db_client
            .indexer()
            .find_pending_transaction_events(max_block_height)
            .await
            .map_err(|e| {
                warn!("Failed to query pending transaction events: {}", e);
                e
            })?;

        info!("Found {} pending transaction events", events.len());

        let withdrawal_events: Vec<WithdrawalEvent> = events
            .into_iter()
            .filter_map(|event| {
                let event_type =
                    match WithdrawalEventType::from_db_string(event.transaction_type.clone()) {
                        Ok(et) => et,
                        Err(e) => {
                            warn!(
                                "Skipping event with invalid type: {} (nonce: {})",
                                e, event.nonce
                            );
                            return None;
                        }
                    };
                let chain_id = event.l1_chain_id as u64;
                let nonce = event.nonce as u64;

                // Record polled event metric
                crate::metrics::record_event_polled(chain_id, nonce, &event_type.to_string());

                Some(WithdrawalEvent {
                    event_type,
                    l1_chain_id: chain_id,
                    l2_transaction_hash: event.l2_transaction_hash.unwrap_or_default(),
                    l1_token: event.l1_token,
                    l1_address: event.l1_address,
                    nonce,
                    height: event.l2_block_height as u64,
                    status: event.handle_status.unwrap_or(0) as u16,
                })
            })
            .collect();

        info!("Converted to {} withdrawal events", withdrawal_events.len());
        Ok(withdrawal_events)
    }

    /// Get the latest block height that is included in a batch
    async fn get_latest_batched_block_height(twine_provider: &TwineProvider) -> Result<u64> {
        // Get the latest batch number
        let latest_batch_number = twine_provider.batch_client.get_latest_batch().await?;

        // Get the latest batch details to understand its block range
        let latest_batch = twine_provider
            .batch_client
            .get_full_batch(latest_batch_number, Some(false))
            .await?;

        let block_range = latest_batch.block_range();
        let latest_block_in_batch = *block_range.end();

        info!(
            "Latest batch {} contains blocks {}..{}, using {} as max block height",
            latest_batch_number,
            block_range.start(),
            latest_block_in_batch,
            latest_block_in_batch
        );

        Ok(latest_block_in_batch)
    }
}
