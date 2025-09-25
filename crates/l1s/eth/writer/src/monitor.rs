use std::time::Duration;

use alloy_primitives::B256;
use alloy_provider::{DynProvider, PendingTransactionConfig, Provider};
use alloy_rpc_types::TransactionReceipt;
use tracing::{debug, warn};

/// Handle to monitor transactions.
#[derive(Debug, Clone)]
pub struct TransactionMonitoringHandle {
    /// Network provider.
    provider: DynProvider,
}

impl TransactionMonitoringHandle {
    /// Creates a new [`TransactionMonitoringHandle`].
    pub fn new(provider: DynProvider) -> Self { Self { provider } }

    /// Attempts to wait for a transaction confirmation.
    pub async fn watch_transaction(
        &self,
        tx_hash: B256,
        timeout: Duration,
    ) -> Option<TransactionReceipt> {
        let config = PendingTransactionConfig::new(tx_hash).with_timeout(Some(timeout));

        let confirmed_via_watch = match self
            .provider
            .watch_pending_transaction(config.clone())
            .await
        {
            Ok(tx) => match tx.await {
                Ok(_) => true,
                Err(err) => {
                    warn!(?err, ?tx_hash, "pending transaction watch failed");
                    false
                }
            },
            Err(err) => {
                debug!(?err, ?tx_hash, "failed to start pending transaction watch");
                false
            }
        };

        match self.provider.get_transaction_receipt(tx_hash).await {
            Ok(Some(receipt)) => Some(receipt),
            Ok(None) if confirmed_via_watch => {
                debug!(
                    ?tx_hash,
                    "transaction confirmed via watch but receipt missing"
                );
                None
            }
            Ok(None) => None,
            Err(err) => {
                warn!(?err, ?tx_hash, "failed to fetch transaction receipt");
                None
            }
        }
    }
}
