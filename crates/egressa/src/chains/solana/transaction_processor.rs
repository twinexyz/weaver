use std::sync::Arc;

use reth_tracing::tracing::{debug, error, info, warn};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_sdk::commitment_config::{CommitmentConfig, CommitmentLevel};
use solana_sdk::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signature, Signer};
use solana_sdk::transaction::Transaction;
use tokio::time::{sleep, Duration};
use twine_l1::error::TransactionError;

/// TransactionProcessor handles the complete lifecycle of transaction
/// processing:
/// - Signing transactions via external signing service
/// - Sending signed transactions to the blockchain
/// - Retry logic for failed transactions
/// - Waiting for transaction confirmation
#[allow(missing_debug_implementations)]
pub struct TransactionProcessor {
    /// Max retries
    pub max_retries: u32,
    /// Retry delay
    pub retry_delay: Duration,
    /// Client
    pub client: Arc<RpcClient>,
    /// Relay signer
    pub relay_signer: Keypair,
    /// Chain ID
    pub chain_id: u64,
}

impl TransactionProcessor {
    /// Create a new TransactionProcessor instance
    pub fn new(
        max_retries: u32,
        retry_delay: Duration,
        client: Arc<RpcClient>,
        relay_signer: Keypair,
        chain_id: u64,
    ) -> Self {
        Self {
            max_retries,
            retry_delay,
            client,
            relay_signer,
            chain_id,
        }
    }

    /// Send a signed transaction to the blockchain
    pub async fn send_signed_transaction(
        &self,
        signed_tx: &Transaction,
    ) -> Result<String, TransactionError> {
        debug!("Broadcasting transaction to network");

        let signature = self
            .client
            .send_transaction_with_config(signed_tx, RpcSendTransactionConfig {
                skip_preflight: false,
                ..Default::default()
            })
            .await
            .map_err(|e| {
                TransactionError::SendError(format!("Failed to broadcast transaction: {}", e))
            })?;

        let signature_str = signature.to_string();
        debug!(
            "✅ Transaction successfully broadcasted with signature: {}",
            signature_str
        );

        Ok(signature_str)
    }

    /// Process a transaction with comprehensive retry logic
    pub async fn process_transaction_with_retry(
        &self,
        mut unsigned_tx: Transaction,
        from_pubkey: Pubkey,
    ) -> Result<String, TransactionError> {
        debug!(
            "Starting transaction processing with retry logic (max retries: {})",
            self.max_retries
        );

        let mut retry_count = 0;
        let mut last_error: Option<TransactionError> = None;

        while retry_count <= self.max_retries {
            match self
                .process_single_transaction(&mut unsigned_tx, &from_pubkey)
                .await
            {
                Ok(signature) => {
                    debug!(
                        "✅ Transaction processed successfully on attempt {}/{}: {}",
                        retry_count + 1,
                        self.max_retries + 1,
                        signature
                    );
                    return Ok(signature);
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
                            self.max_retries as i32,
                            e.to_string(),
                        ));
                    }

                    // Update blockhash and re-sign for retry
                    match self.client.get_latest_blockhash().await {
                        Ok(blockhash) => {
                            unsigned_tx.message.recent_blockhash = blockhash;
                            unsigned_tx.sign(&[&self.relay_signer], blockhash);
                        }
                        Err(blockhash_err) => {
                            warn!("Failed to update blockhash for retry: {}", blockhash_err);
                        }
                    }

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

        Err(last_error.clone().unwrap_or_else(|| {
            TransactionError::MaxRetriesExceeded(
                self.max_retries as i32,
                last_error
                    .unwrap_or_else(|| {
                        TransactionError::MaxRetriesExceeded(
                            self.max_retries as i32,
                            "Unknown error".to_string(),
                        )
                    })
                    .to_string(),
            )
        }))
    }

    /// Process a single transaction attempt (sign + send)
    async fn process_single_transaction(
        &self,
        unsigned_tx: &mut Transaction,
        _from_pubkey: &Pubkey,
    ) -> Result<String, TransactionError> {
        debug!("Processing single transaction attempt");

        // Send the signed transaction
        let signature = self.send_signed_transaction(unsigned_tx).await?;

        debug!(
            "Single transaction processing completed successfully: {}",
            signature
        );
        Ok(signature)
    }

    /// Wait for transaction confirmation
    pub async fn wait_for_confirmation(&self, signature_str: &str) -> Result<(), TransactionError> {
        debug!("⏳ Waiting for transaction confirmation: {}", signature_str);

        let signature = signature_str.parse::<Signature>().map_err(|e| {
            error!("Invalid signature format: {}", e);
            TransactionError::SendError(format!("Invalid signature format: {}", e))
        })?;

        let mut attempts = 0;
        let max_attempts = 60; // 2 minutes at 2-second intervals
        let poll_interval = Duration::from_secs(2);

        loop {
            attempts += 1;

            match self
                .client
                .get_signature_status_with_commitment(&signature, CommitmentConfig {
                    commitment: CommitmentLevel::Finalized,
                })
                .await
            {
                Ok(Some(status)) => match status {
                    Ok(_) => {
                        info!(
                            "✅ Transaction confirmed successfully after {} attempts: {}",
                            attempts, signature_str
                        );
                        return Ok(());
                    }
                    Err(e) => {
                        error!("❌ Transaction failed on-chain: {:?}", e);
                        return Err(TransactionError::ReceiptError(format!(
                            "Transaction failed on-chain: {:?}",
                            e
                        )));
                    }
                },
                Ok(None) => {
                    // Transaction is still pending
                    if attempts > max_attempts {
                        error!(
                            "⏰ Transaction confirmation timeout after {} attempts ({} seconds): {}",
                            max_attempts,
                            max_attempts * poll_interval.as_secs(),
                            signature_str
                        );
                        return Err(TransactionError::ReceiptTimeout);
                    }

                    if attempts % 10 == 0 {
                        debug!(
                            "⏳ Still waiting for confirmation... Attempt {}/{}: {}",
                            attempts, max_attempts, signature_str
                        );
                    }

                    sleep(poll_interval).await;
                }
                Err(e) => {
                    error!(
                        "❌ Error checking transaction status (attempt {}): {}: {}",
                        attempts, e, signature_str
                    );
                    return Err(TransactionError::ReceiptError(format!(
                        "Error checking transaction status (attempt {}): {}: {}",
                        attempts, e, signature_str
                    )));
                }
            }
        }
    }

    /// Complete transaction processing pipeline: sign, send, and confirm
    pub async fn process_and_confirm_transaction(
        &self,
        instruction: Instruction,
        from_pubkey: Pubkey,
        should_wait_for_confirmation: bool,
    ) -> Result<String, TransactionError> {
        debug!(
            "🚀 Starting complete transaction processing pipeline - From: {}",
            from_pubkey
        );

        let recent = self.client.get_latest_blockhash().await.map_err(|e| {
            error!("Failed to get latest blockhash: {}", e);
            TransactionError::ReceiptError(format!("Failed to get latest blockhash: {}", e))
        })?;

        info!("Latest blockhash: {:?}", recent.to_string());

        let payer = &self.relay_signer;
        let payer_pubkey = payer.pubkey();
        let mut tx = Transaction::new_with_payer(&[instruction], Some(&payer_pubkey));

        tx.message.recent_blockhash = recent;

        tx.sign(&[&self.relay_signer], tx.message.recent_blockhash);

        // Step 1: Process transaction with retry logic
        let signature = self.process_transaction_with_retry(tx, from_pubkey).await?;

        // Step 2: Wait for confirmation
        if should_wait_for_confirmation {
            self.wait_for_confirmation(&signature).await?;
        }

        debug!(
            "🎉 Transaction processing pipeline completed successfully: {}",
            signature
        );
        Ok(signature)
    }

    /// Calculate retry delay with exponential backoff
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

    /// Get transaction processing statistics
    pub fn get_processing_config(&self) -> ProcessingConfig {
        ProcessingConfig {
            max_retries: self.max_retries,
            base_retry_delay: self.retry_delay,
            confirmation_timeout_seconds: 120,
            poll_interval_seconds: 2,
        }
    }
}

/// Configuration information for transaction processing
#[derive(Debug, Clone)]
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
