use std::time::Duration;

use alloy_primitives::TxHash;
use alloy_rpc_types::TransactionReceipt;
use tokio::time::sleep;
use twine_types::L1Action;

use crate::EthTxManager;

impl EthTxManager {
    pub(crate) async fn start_receipt_status_poller(&self, chain_id: u64) {
        let manager = self.clone();
        let handle = tokio::spawn(async move {
            loop {
                if let Err(e) = manager.poll_pending_transactions(chain_id).await {
                    tracing::error!("Poller error: {}", e);
                    // Exponential backoff on errors
                    sleep(Duration::from_secs(10)).await;
                } else {
                    // Normal polling interval
                    sleep(Duration::from_secs(5)).await;
                }
            }
        });

        let mut guard = self.poller_handle.lock().await;
        *guard = Some(handle);
    }

    async fn poll_pending_transactions(&self, chain_id: u64) -> eyre::Result<()> {
        let pending_txs = self.connection.get_pending_transactions(chain_id).await?;

        for (tx_hash, _, _, action, batch_number) in pending_txs {
            match self.eth_sender.get_receipt(tx_hash).await {
                Ok(Some(receipt)) => {
                    self.handle_receipt(tx_hash, receipt, action, batch_number)
                        .await?;
                }
                Ok(None) => continue, // Still pending
                Err(e) => {
                    tracing::warn!("Failed to get receipt for {}: {}", tx_hash, e);
                    continue;
                }
            }
        }
        Ok(())
    }

    async fn handle_receipt(
        &self,
        tx_hash: TxHash,
        receipt: TransactionReceipt,
        action: L1Action,
        batch_number: u64,
    ) -> eyre::Result<()> {
        if receipt.status() {
            self.connection
                .mark_confirmed(
                    tx_hash,
                    receipt.block_number.unwrap_or(0),
                    receipt.gas_used,
                    receipt.effective_gas_price as u64,
                )
                .await?;

            self.post_confirmation_hook(action, batch_number).await?;
        } else {
            self.connection.mark_failed(tx_hash).await?;
            self.post_failure_hook(action, batch_number).await?;
        }
        Ok(())
    }

    /// TODO: Might have to extend later
    async fn post_confirmation_hook(
        &self,
        action: L1Action,
        batch_number: u64,
    ) -> eyre::Result<()> {
        match action {
            L1Action::CommitBatch => {
                tracing::info!(batch_number, "Batch commited");
            }
            L1Action::FinalizeBatch => {
                tracing::info!(batch_number, "Batch finalized");
            }
            L1Action::FinalizeTransactions => {
                tracing::info!(batch_number, "Transactions finalized");
            }
        }
        Ok(())
    }

    /// TODO: Might have to extend later
    async fn post_failure_hook(&self, action: L1Action, batch_number: u64) -> eyre::Result<()> {
        match action {
            L1Action::CommitBatch => {
                tracing::info!(batch_number, "Batch commit failed");
            }
            L1Action::FinalizeBatch => {
                tracing::info!(batch_number, "Batch finalize failed");
            }
            L1Action::FinalizeTransactions => {
                tracing::info!(batch_number, "Transactions finalize failed");
            }
        }
        Ok(())
    }
}
