pub mod ethereum;
pub mod factory;
pub mod solana;

use alloy_primitives::Bytes;
use async_trait::async_trait;
use eyre::Result;

/// L1 transaction sender
#[async_trait]
pub trait L1TransactionSender: Send + Sync {
    /// Execute forced withdrawal
    async fn execute_forced_withdrawal(
        &self,
        public_values: Bytes,
        withdrawal_proof: Bytes,
    ) -> Result<String>;

    /// Execute L2 withdraw
    async fn execute_l2_withdraw(
        &self,
        public_values: Bytes,
        withdraw_proof: Bytes,
    ) -> Result<String>;

    /// Refund deposit
    async fn refund_deposit(&self, public_values: Bytes, refund_proof: Bytes) -> Result<String>;
}
