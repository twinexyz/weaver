use async_trait::async_trait;
use eyre::Result;
use reth_tracing::tracing::{info, warn};

use crate::find_pending_transaction_events;
use crate::types::{WithdrawalEvent, WithdrawalEventType};

/// Dummy polling service for testing
#[derive(Debug)]
pub struct WithdrawalEventPoller;

impl WithdrawalEventPoller {
    pub async fn poll_events(
        &self,
        indexer_db_pool: sqlx::PgPool,
        _db_pool: sqlx::PgPool,
    ) -> Result<Vec<WithdrawalEvent>> {
        info!("Polling for pending transaction events from database...");

        // Query the database for pending events
        let events = find_pending_transaction_events(&indexer_db_pool)
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
                let l2_transaction_hash = match event_type {
                    WithdrawalEventType::L2Withdraw => event.transaction_hash.unwrap_or_default(),
                    _ => event.handle_tx_hash.unwrap_or_default(),
                };

                let chain_id = match event_type {
                    WithdrawalEventType::L2Withdraw =>
                        event.destination_chain_id.unwrap_or(0) as u64,
                    _ => event.chain_id as u64,
                };

                let height = match event_type {
                    WithdrawalEventType::L2Withdraw => event.block_number,
                    _ => event.handle_block_number.unwrap_or(0),
                };

                WithdrawalEvent {
                    event_type,
                    chain_id,
                    l2_transaction_hash,
                    l1_token: event.l1_token,
                    l1_address: event.l1_address,
                    nonce: event.nonce as u64,
                    height: height as u64,
                    status: event.handle_status.unwrap_or(0) as u16,
                }
            })
            .collect();

        info!("Converted to {} withdrawal events", withdrawal_events.len());
        Ok(withdrawal_events)
    }
}
