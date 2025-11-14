use reth_tracing::tracing::{error, info, warn};

use crate::chains::factory::L1SenderFactory;
use crate::chains::twine::provider::TwineProvider;
use crate::database::client::DbClient;
use crate::types::{WithdrawalEvent, WithdrawalEventType};

#[derive(Debug)]
pub struct WithdrawalEventPoller;

impl WithdrawalEventPoller {
    /// Poll for pending transaction events from indexer database
    pub async fn poll_events(
        &self,
        db_client: DbClient,
        twine_provider: &TwineProvider,
        l1_sender_factory: &L1SenderFactory,
    ) -> eyre::Result<Vec<WithdrawalEvent>> {
        info!("Polling for pending transaction events from database...");

        let mut all_withdrawal_events: Vec<WithdrawalEvent> = Vec::new();

        for chain_config in l1_sender_factory.get_l1_chains() {
            // Get sender - continue on error instead of returning
            let l1_sender = match l1_sender_factory
                .get_l1_provider(chain_config.chain_id)
                .await
            {
                Some(sender) => sender,
                None => {
                    warn!(
                        "Failed to get L1 sender for chain: {}, skipping",
                        chain_config.chain_id
                    );
                    continue;
                }
            };
            let last_finalized_batch = match l1_sender.get_last_finalized_batch().await {
                Ok(batch) => {
                    if batch == 0 {
                        warn!(
                            "Chain {} has no finalized batches yet, skipping",
                            chain_config.chain_id
                        );
                        continue;
                    }
                    batch
                }
                Err(e) => {
                    error!(
                        "Failed to get finalized batch for chain {}: {}, skipping",
                        chain_config.chain_id, e
                    );
                    continue;
                }
            };

            info!(
                "Last finalized batch for chain {}: {}",
                chain_config.chain_id, last_finalized_batch
            );

            // Get the latest batch information to filter events
            let max_block_height = match self
                .get_max_block_height_for_batch(twine_provider, last_finalized_batch)
                .await
            {
                Ok(height) => {
                    info!(
                        "Max block height for finalized batch {} on chain {}: {}",
                        last_finalized_batch, chain_config.chain_id, height
                    );
                    height
                }
                Err(e) => {
                    error!(
                        "Failed to get max block height for batch {} on chain {}: {}, skipping",
                        last_finalized_batch, chain_config.chain_id, e
                    );
                    continue;
                }
            };

            // Query the database for pending events with block height filter
            let events = match db_client
                .indexer()
                .find_pending_transaction_events(chain_config.chain_id, max_block_height)
                .await
            {
                Ok(events) => events,
                Err(e) => {
                    warn!(
                        "Failed to query pending transaction events for chain {}: {}, skipping",
                        chain_config.chain_id, e
                    );
                    continue;
                }
            };

            info!(
                "Found {} pending transaction events for chain {}: {}",
                events.len(),
                chain_config.chain_id,
                max_block_height
            );

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

            all_withdrawal_events.extend(withdrawal_events);
        }

        info!(
            "Converted to {} withdrawal events",
            all_withdrawal_events.len()
        );
        Ok(all_withdrawal_events)
    }

    // /// Get the latest block height that is included in a batch
    // async fn get_latest_batched_block_height(twine_provider: &TwineProvider) ->
    // Result<u64> {     // Get the latest batch number
    //     let latest_batch_number =
    // twine_provider.batch_client.get_latest_batch().await?;

    //     // Get the latest batch details to understand its block range
    //     let latest_batch = twine_provider
    //         .batch_client
    //         .get_full_batch(latest_batch_number, Some(false))
    //         .await?;

    //     let block_range = latest_batch.block_range();
    //     let latest_block_in_batch = *block_range.end();

    //     info!(
    //         "Latest batch {} contains blocks {}..{}, using {} as max block
    // height",         latest_batch_number,
    //         block_range.start(),
    //         latest_block_in_batch,
    //         latest_block_in_batch
    //     );

    //     Ok(latest_block_in_batch)
    // }

    /// Get the latest block height for a given batch number
    async fn get_max_block_height_for_batch(
        &self,
        twine_provider: &TwineProvider,
        batch_number: u64,
    ) -> eyre::Result<u64> {
        // Get the latest batch number
        let batch = twine_provider
            .batch_client
            .get_full_batch(batch_number, Some(false))
            .await
            .map_err(|e| eyre::eyre!("Failed to get full batch: {}", e))?;
        let block_range = batch.block_range();
        let max_block_height = *block_range.end();
        Ok(max_block_height)
    }
}
