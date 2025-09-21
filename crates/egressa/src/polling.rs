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
                event_type: WithdrawalEventType::ForcedWithdraw,
                chain_id: 1,
                l2_transaction_hash: "0x1234567890abcdef".to_string(),
                l1_token: "0xTokenAddress".to_string(),
                l1_address: "0xUserAddress".to_string(),
                public_values: Bytes::from(vec![1, 2, 3, 4]),
                proof: Bytes::from(vec![5, 6, 7, 8]),
            },
            WithdrawalEvent {
                event_type: WithdrawalEventType::L2Withdraw,
                chain_id: 1,
                l2_transaction_hash: "0xabcdef1234567890".to_string(),
                l1_token: "0xTokenAddress2".to_string(),
                l1_address: "0xUserAddress2".to_string(),
                public_values: Bytes::from(vec![9, 10, 11, 12]),
                proof: Bytes::from(vec![13, 14, 15, 16]),
            },
            WithdrawalEvent {
                event_type: WithdrawalEventType::RefundDeposit,
                chain_id: 137, // Polygon
                l2_transaction_hash: "0xfedcba0987654321".to_string(),
                l1_token: "0xTokenAddress3".to_string(),
                l1_address: "0xUserAddress3".to_string(),
                public_values: Bytes::from(vec![17, 18, 19, 20]),
                proof: Bytes::from(vec![21, 22, 23, 24]),
            },
        ]
    }
}
