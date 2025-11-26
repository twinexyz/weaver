//! Solana chain state provider implementation

use std::fmt::Debug;

use alloy_primitives::FixedBytes;
use async_trait::async_trait;
use twine_l1_solana::SolanaProvider;

use crate::chain_watcher::watcher::ChainStateProvider;
use crate::common::consts::SOLANA_CHAIN_IDENTIFIER;
use crate::common::db_strings::DBStrings;
use crate::config::config::L1Config;
use crate::errors::TwineSequencerError;

/// Solana chain state provider
pub struct SolanaChainProvider {
    /// Solana provider for querying chain state
    provider: SolanaProvider,
    /// Polling interval in milliseconds
    poll_interval: u64,
}

impl Debug for SolanaChainProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SolanaChainProvider")
            .field("provider", &self.provider)
            .finish()
    }
}

#[async_trait]
impl ChainStateProvider for SolanaChainProvider {
    type Config = L1Config;

    async fn from_config(config: Self::Config) -> Result<Self, TwineSequencerError> {
        let provider = SolanaProvider::new(
            config.rpc_url,
            config.chain_id,
            &config.bridge_contract_address,
            String::new(), // No admin wallet needed for querying
        );

        Ok(Self {
            provider,
            poll_interval: config.poll_interval,
        })
    }

    fn chain_id(&self) -> &str { SOLANA_CHAIN_IDENTIFIER }

    fn db_config(&self) -> DBStrings { DBStrings::for_chain(SOLANA_CHAIN_IDENTIFIER) }

    fn poll_interval(&self) -> u64 { self.poll_interval }

    fn log_target(&self) -> &str { "solana_watcher" }

    async fn fetch_batch_hash(
        &self,
        batch_number: u64,
    ) -> Result<FixedBytes<32>, TwineSequencerError> {
        // Query the Solana L1 for the current twine chain storage
        let twine_chain_storage = self.provider.get_twine_chain_storage().await.map_err(|e| {
            TwineSequencerError::Other(format!(
                "Failed to query Solana L1 for twine chain storage: {}",
                e
            ))
        })?;

        let batch_hash = if batch_number == twine_chain_storage.last_committed_batch_number {
            // The requested batch matches the last committed batch
            FixedBytes::<32>::from(twine_chain_storage.last_committed_batch_hash)
        } else if batch_number > twine_chain_storage.last_committed_batch_number {
            // The requested batch hasn't been committed yet
            tracing::debug!(
                target = "solana_watcher",
                requested_batch = batch_number,
                last_committed_batch = twine_chain_storage.last_committed_batch_number,
                "requested batch not committed yet"
            );
            FixedBytes::<32>::ZERO
        } else {
            // The requested batch is older than the last committed batch
            // Query the individual commitment PDA for this batch
            match self
                .provider
                .get_batch_hash_from_commitment_pda(batch_number)
                .await
            {
                Ok(hash) => FixedBytes::<32>::from(hash),
                Err(e) => {
                    tracing::warn!(
                        target = "solana_watcher",
                        requested_batch = batch_number,
                        error = ?e,
                        "failed to get batch hash from commitment PDA"
                    );
                    return Err(TwineSequencerError::Other(e.to_string()));
                }
            }
        };

        Ok(batch_hash)
    }
}
