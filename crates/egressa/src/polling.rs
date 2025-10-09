use eyre::Result;
use reth_tracing::tracing::{info, warn};

use crate::database::client::DbClient;
use crate::types::{WithdrawalEvent, WithdrawalEventType};

/// Dummy polling service for testing
#[derive(Debug)]
pub struct WithdrawalEventPoller;

impl WithdrawalEventPoller {
    /// Poll for pending transaction events from indexer database
    pub async fn poll_events(&self, db_client: DbClient) -> Result<Vec<WithdrawalEvent>> {
        info!("Polling for pending transaction events from database...");

        // Query the database for pending events
        let events = db_client
            .indexer()
            .find_pending_transaction_events()
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
}
