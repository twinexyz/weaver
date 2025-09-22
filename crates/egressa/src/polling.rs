use alloy_primitives::Bytes;
use async_trait::async_trait;

use crate::types::{WithdrawalEvent, WithdrawalEventPoller, WithdrawalEventType};

/// Dummy polling service for testing
pub struct DummyWithdrawalEventPoller;

#[async_trait]
impl WithdrawalEventPoller for DummyWithdrawalEventPoller {
    async fn poll_events(&self) -> Vec<WithdrawalEvent> {
        // Return dummy events for testing
        vec![
            WithdrawalEvent {
                event_type: WithdrawalEventType::L2Withdraw,
                chain_id: 11155111,
                l2_transaction_hash: "0xbc36533aea1b5ffb18cc1b0b088e05c24496726497fd43c931861535f5e91e71".to_string(),
                l1_token: "0xTokenAddress".to_string(),
                l1_address: "0xUserAddress".to_string(),
                nonce: 1,
            },
            WithdrawalEvent {
                event_type: WithdrawalEventType::L2Withdraw,
                chain_id: 11155111,
                l2_transaction_hash: "0x395c3fea31a1b11d30289aa2eb21d9cd240d7544146ff57048351007fb4e471e".to_string(),
                l1_token: "0xTokenAddress2".to_string(),
                l1_address: "0xUserAddress2".to_string(),
                nonce: 2,
            },
        ]
    }
}
