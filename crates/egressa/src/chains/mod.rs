pub mod ethereum;
pub mod factory;
pub mod solana;
pub mod twine;

use alloy_primitives::Bytes;
use async_trait::async_trait;
use eyre::Result;

use crate::{WithdrawalEvent, WithdrawalEventType};

/// L1 transaction sender
#[async_trait]
pub trait L1TransactionSender: Send + Sync {
    async fn handle_event(
        &self,
        event: WithdrawalEvent,
        public_values: Bytes,
        zk_proof: Bytes,
    ) -> Result<String> {
        match event.event_type {
            WithdrawalEventType::ForcedWithdraw =>
                self.execute_forced_withdrawal(event, public_values, zk_proof)
                    .await,
            WithdrawalEventType::L2Withdraw =>
                self.execute_l2_withdraw(event, public_values, zk_proof)
                    .await,
            WithdrawalEventType::RefundDeposit =>
                self.refund_deposit(event, public_values, zk_proof).await,
        }
    }

    /// Execute forced withdrawal
    async fn execute_forced_withdrawal(
        &self,
        event: WithdrawalEvent,
        public_values: Bytes,
        zk_proof: Bytes,
    ) -> Result<String>;

    /// Execute L2 withdraw
    async fn execute_l2_withdraw(
        &self,
        event: WithdrawalEvent,
        public_values: Bytes,
        zk_proof: Bytes,
    ) -> Result<String>;

    /// Refund deposit
    async fn refund_deposit(
        &self,
        event: WithdrawalEvent,
        public_values: Bytes,
        zk_proof: Bytes,
    ) -> Result<String>;
}
