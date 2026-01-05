use std::str::FromStr;
use std::sync::Arc;

use alloy_primitives::TxHash;
use alloy_provider::{DynProvider, Provider};
use alloy_rpc_types::TransactionRequest;
use eyre::Result;
use reth_tracing::tracing::{debug, error, info, warn};
use tokio::time::{sleep, Duration};
use tracing::instrument;
use twine_l1_eth::twine_l1_eth_reader::clients::execution::EthQueryExecutionClient;

/// Transaction processing errors
#[derive(Debug, Clone, thiserror::Error)]
#[allow(missing_docs)]
pub enum TransactionError {
    #[error("Process deposit error: {0}")]
    ProcessDepositError(String),
    #[error("Max retries exceeded: {0} Error: {1}")]
    MaxRetriesExceeded(u32, String),
    #[error("Receipt timeout")]
    ReceiptTimeout,
}

/// `TransactionProcessor` handles the complete lifecycle of transaction
/// processing:
/// - Sending signed transactions to the blockchain
/// - Retry logic for failed transactions
/// - Waiting for transaction confirmation
#[allow(missing_docs)]
#[derive(Clone)]
pub struct TransactionProcessor {
    pub query_client: EthQueryExecutionClient,
    pub max_retries: u32,
    pub retry_delay: Duration,
    pub provider: Arc<DynProvider>,
    pub chain_id: u64,
}

impl TransactionProcessor {
    /// Create a new `TransactionProcessor` instance
    pub fn new(
        query_client: EthQueryExecutionClient,
        max_retries: u32,
        retry_delay: Duration,
        provider: Arc<DynProvider>,
        chain_id: u64,
    ) -> Self {
        Self {
            query_client,
            max_retries,
            retry_delay,
            provider,
            chain_id,
        }
    }

    /// Process a transaction with comprehensive retry logic
    ///
    /// This method:
    /// 1. Attempts to sign and send the transaction
    /// 2. Retries on failure with exponential backoff
    /// 3. Logs detailed information about retry attempts
    /// 4. Returns the final transaction hash or error after max retries
    pub async fn process_transaction_with_retry(
        &self,
        unsigned_tx: TransactionRequest,
    ) -> Result<String, TransactionError> {
        debug!(
            "Starting transaction processing with retry logic (max retries: {})",
            self.max_retries
        );

        let mut retry_count = 0;
        let mut last_error: Option<TransactionError> = None;

        while retry_count <= self.max_retries {
            match self.process_single_transaction(&unsigned_tx).await {
                Ok(tx_hash) => {
                    debug!(
                        "✅ Transaction processed successfully on attempt {}/{}: {}",
                        retry_count + 1,
                        self.max_retries + 1,
                        tx_hash
                    );
                    return Ok(tx_hash);
                }
                Err(e) => {
                    last_error = Some(e.clone());
                    retry_count += 1;

                    if retry_count > self.max_retries {
                        error!(
                            "❌ Transaction failed after {} attempts. Final error: {}",
                            self.max_retries + 1,
                            e
                        );
                        return Err(TransactionError::MaxRetriesExceeded(
                            self.max_retries,
                            e.to_string(),
                        ));
                    }

                    // Calculate delay with exponential backoff
                    let delay = self.calculate_retry_delay(retry_count);

                    warn!(
                        "⚠️ Transaction attempt {}/{} failed: {}. Retrying in {:?}...",
                        retry_count,
                        self.max_retries + 1,
                        e,
                        delay
                    );

                    sleep(delay).await;
                }
            }
        }

        // This should never be reached due to the logic above, but just in case
        Err(last_error.clone().unwrap_or_else(|| {
            TransactionError::MaxRetriesExceeded(
                self.max_retries,
                last_error
                    .unwrap_or_else(|| {
                        TransactionError::MaxRetriesExceeded(
                            self.max_retries,
                            "Unknown error".to_string(),
                        )
                    })
                    .to_string(),
            )
        }))
    }

    /// Process a single transaction attempt (sign + send)
    ///
    /// This is a helper method that combines signing and sending into one
    /// atomic operation
    async fn process_single_transaction(
        &self,
        unsigned_tx: &TransactionRequest,
    ) -> Result<String, TransactionError> {
        debug!("Processing single transaction attempt");

        let tx = self
            .provider
            .send_transaction(unsigned_tx.clone())
            .await
            .map_err(|e| {
                error!("Failed to send transaction: {}", e);
                TransactionError::ProcessDepositError(format!("Failed to send transaction: {e}"))
            })?;

        let tx_hash = tx.tx_hash();

        debug!(
            "Single transaction processing completed successfully: {}",
            tx_hash.to_string()
        );
        Ok(tx_hash.to_string())
    }

    /// Wait for transaction confirmation with comprehensive polling
    ///
    /// This method:
    /// 1. Polls the blockchain for transaction receipt
    /// 2. Implements timeout logic to prevent infinite waiting
    /// 3. Provides detailed logging about confirmation status
    /// 4. Handles various receipt states (pending, confirmed, failed)
    pub async fn wait_for_confirmation(&self, tx_hash: &str) -> Result<(), TransactionError> {
        debug!("⏳ Waiting for transaction confirmation: {}", tx_hash);

        let hash = TxHash::from_str(tx_hash).map_err(|e| {
            error!("Invalid transaction hash format: {}", e);
            TransactionError::ProcessDepositError(format!("Invalid tx hash format: {e}"))
        })?;

        let mut attempts = 0;
        let max_attempts = 120; // 4 minutes at 5-second intervals
        let poll_interval = Duration::from_secs(5);

        loop {
            attempts += 1;

            match self.query_client.get_tx_receipt(hash).await {
                Ok(Some(receipt)) => {
                    // Check if transaction was successful
                    if receipt.status() {
                        info!(
                            "✅ Transaction confirmed successfully after {} attempts: {}",
                            attempts, tx_hash
                        );
                        info!(
                            "📊 Receipt details - Block: {:?}, Gas used: {:?}",
                            receipt.block_number, receipt.gas_used
                        );
                        return Ok(());
                    }
                    error!(
                        "❌ Transaction failed (reverted) in block {:?}: {}",
                        receipt.block_number, tx_hash
                    );
                    return Err(TransactionError::ProcessDepositError(
                        "Transaction was mined but reverted".to_string(),
                    ));
                }
                Ok(None) => {
                    // Transaction is still pending
                    if attempts > max_attempts {
                        error!(
                            "⏰ Transaction confirmation timeout after {} attempts ({} seconds): {}",
                            max_attempts,
                            max_attempts * poll_interval.as_secs(),
                            tx_hash
                        );
                        return Err(TransactionError::ReceiptTimeout);
                    }

                    if attempts.is_multiple_of(10) {
                        debug!(
                            "⏳ Still waiting for confirmation... Attempt {}/{}: {}",
                            attempts, max_attempts, tx_hash
                        );
                    }

                    sleep(poll_interval).await;
                }
                Err(e) => {
                    error!(
                        "❌ Error checking transaction receipt (attempt {}): {}: {}",
                        attempts, e, tx_hash
                    );
                    return Err(TransactionError::ProcessDepositError(format!(
                        "Error checking receipt: {e}"
                    )));
                }
            }
        }
    }

    /// Complete transaction processing pipeline: sign, send, and confirm
    ///
    /// This is the main entry point that combines all transaction processing
    /// steps:
    /// 1. Process transaction with retry logic
    /// 2. Wait for confirmation
    /// 3. Return final result

    #[instrument(skip_all, fields(chain_id = %self.chain_id))]
    pub async fn process_and_confirm_transaction(
        &self,
        unsigned_tx: TransactionRequest,
        should_wait_for_confirmation: bool,
    ) -> Result<String, TransactionError> {
        // Step 1: Process transaction with retry logic
        let tx_hash = self.process_transaction_with_retry(unsigned_tx).await?;

        // Step 2: Wait for confirmation
        if should_wait_for_confirmation {
            self.wait_for_confirmation(&tx_hash).await?;
        }

        debug!(
            "🎉 Transaction processing pipeline completed successfully: {}",
            tx_hash
        );
        Ok(tx_hash)
    }

    /// Calculate retry delay with exponential backoff
    ///
    /// This method implements a smart retry strategy:
    /// - Base delay from configuration
    /// - Exponential backoff for subsequent retries
    /// - Maximum cap to prevent excessive delays
    fn calculate_retry_delay(&self, retry_count: u32) -> Duration {
        if retry_count == 0 {
            return Duration::from_millis(0);
        }

        let base_delay_ms = self.retry_delay.as_millis() as u64;
        let exponential_factor = 2_u64.saturating_pow(retry_count - 1);
        let calculated_delay_ms = base_delay_ms * exponential_factor;

        let max_delay_ms = 60_000;
        let final_delay_ms = calculated_delay_ms.min(max_delay_ms);

        Duration::from_millis(final_delay_ms)
    }

    /// Get transaction processing statistics (useful for monitoring)
    pub fn get_processing_config(&self) -> ProcessingConfig {
        ProcessingConfig {
            max_retries: self.max_retries,
            base_retry_delay: self.retry_delay,
            confirmation_timeout_seconds: 120, // 2 minutes
            poll_interval_seconds: 2,
        }
    }
}

/// Configuration information for transaction processing
#[derive(Debug, Clone)]
#[allow(missing_docs)]
pub struct ProcessingConfig {
    pub max_retries: u32,
    pub base_retry_delay: Duration,
    pub confirmation_timeout_seconds: u64,
    pub poll_interval_seconds: u64,
}

impl std::fmt::Display for ProcessingConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ProcessingConfig {{ max_retries: {}, base_retry_delay: {:?}, confirmation_timeout: {}s, poll_interval: {}s }}",
            self.max_retries,
            self.base_retry_delay,
            self.confirmation_timeout_seconds,
            self.poll_interval_seconds
        )
    }
}

impl std::fmt::Debug for TransactionProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransactionProcessor")
            .field("query_client", &self.query_client)
            .field("max_retries", &self.max_retries)
            .field("retry_delay", &self.retry_delay)
            .field("provider", &"<dyn Provider>")
            .field("chain_id", &self.chain_id)
            .finish()
    }
}
