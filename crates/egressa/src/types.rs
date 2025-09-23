use alloy_primitives::Bytes;

use crate::TransactionEventType;

/// Withdrawal event types
#[derive(Debug, Clone)]
pub enum WithdrawalEventType {
    ForcedWithdraw,
    L2Withdraw,
    RefundDeposit,
}

impl WithdrawalEventType {
    pub fn from_db_string(event_type: String) -> Self {
        match event_type.as_str() {
            "ForcedWithdraw" => Self::ForcedWithdraw,
            "Withdraw" => Self::L2Withdraw,
            "Deposit" => Self::RefundDeposit,
            _ => panic!("Invalid withdrawal event type: {}", event_type),
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            Self::ForcedWithdraw => "ForcedWithdraw".to_string(),
            Self::L2Withdraw => "Withdraw".to_string(),
            Self::RefundDeposit => "Deposit".to_string(),
        }
    }
}

/// Withdrawal event from the database
#[derive(Debug, Clone)]
pub struct WithdrawalEvent {
    pub event_type: WithdrawalEventType,
    pub chain_id: u64,
    pub l2_transaction_hash: String,
    pub l1_token: String,
    pub l1_address: String,
    pub nonce: u64,
    pub status: u16,
    pub height: u64,
}
