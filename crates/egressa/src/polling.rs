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
            .map(|event| {
                let event_type = WithdrawalEventType::from_db_string(event.transaction_type);

                WithdrawalEvent {
                    event_type,
                    l1_chain_id: event.l1_chain_id as u64,
                    l2_transaction_hash: event.l2_transaction_hash.unwrap_or_default(),
                    l1_token: event.l1_token,
                    l1_address: event.l1_address,
                    nonce: event.nonce as u64,
                    height: event.l2_block_height as u64,
                    status: event.handle_status.unwrap_or(0) as u16,
                }
            })
            .collect();

        info!("Converted to {} withdrawal events", withdrawal_events.len());
        Ok(withdrawal_events)
    }
}
