use alloy_primitives::Bytes;

/// Withdrawal event types
#[derive(Debug, Clone)]
pub enum WithdrawalEventType {
    ForcedWithdraw,
    L2Withdraw,
    RefundDeposit,
}

/// Withdrawal event from the database
#[derive(Debug, Clone)]
pub struct WithdrawalEvent {
    pub event_type: WithdrawalEventType,
    pub chain_id: u64,
    pub l2_transaction_hash: String,
    pub l1_token: String,
    pub l1_address: String,
    pub public_values: Bytes,
    pub proof: Bytes,
}

/// Polling service trait for getting withdrawal events
#[async_trait::async_trait]
pub trait WithdrawalEventPoller {
    async fn poll_events(&self) -> Vec<WithdrawalEvent>;
}
