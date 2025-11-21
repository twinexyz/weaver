//! Ethereum chain state provider implementation

use std::fmt::Debug;

use alloy_primitives::FixedBytes;
use alloy_rpc_types::TransactionRequest;
use alloy_sol_types::SolCall;
use async_trait::async_trait;
use twine_evm_contracts::twine_chain::TwineChain;
use twine_l1_eth::twine_l1_eth_reader::{EthReader, EthReaderBuilder};

use crate::chain_watcher::watcher::ChainStateProvider;
use crate::common::consts::ETHEREUM_CHAIN_IDENTIFIER;
use crate::common::db_strings::DBStrings;
use crate::config::config::L1Config;
use crate::errors::TwineSequencerError;

/// Ethereum chain state provider
pub struct EthereumChainProvider {
    /// Ethereum RPC client
    client: EthReader,
    /// Bridge contract address on Ethereum
    twine_chain_address: alloy_primitives::Address,
}

impl Debug for EthereumChainProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EthereumChainProvider")
            .field("twine_chain_address", &self.twine_chain_address)
            .finish()
    }
}

#[async_trait]
impl ChainStateProvider for EthereumChainProvider {
    type Config = L1Config;

    async fn from_config(config: Self::Config) -> Result<Self, TwineSequencerError> {
        let twine_chain_address: alloy_primitives::Address =
            config.twine_chain_address.parse().map_err(|e| {
                TwineSequencerError::Other(format!("Invalid twine chain address: {}", e))
            })?;

        let client = EthReaderBuilder::new()
            .with_execution_rpc(config.rpc_url)
            .build()
            .await
            .map_err(|e| TwineSequencerError::Other(e.to_string()))?;

        Ok(Self {
            client,
            twine_chain_address,
        })
    }

    fn chain_id(&self) -> &str { ETHEREUM_CHAIN_IDENTIFIER }

    fn db_config(&self) -> DBStrings { DBStrings::for_chain(ETHEREUM_CHAIN_IDENTIFIER) }

    fn log_target(&self) -> &str { "eth_watcher" }

    async fn fetch_batch_hash(
        &self,
        batch_number: u64,
    ) -> Result<FixedBytes<32>, TwineSequencerError> {
        let execution_client = self.client.execution.as_ref().ok_or_else(|| {
            TwineSequencerError::Other("Execution client not initialized".to_string())
        })?;

        let call = TwineChain::committedBatchCall { 0: batch_number };
        let calldata = call.abi_encode();

        let tx = TransactionRequest::default()
            .to(self.twine_chain_address)
            .input(calldata.into());

        let result = execution_client
            .call_contract(&tx, None)
            .await
            .map_err(|e| {
                TwineSequencerError::Other(format!(
                    "Failed to call bridge for batch {}: {}",
                    batch_number, e
                ))
            })?;

        if result.0.is_empty() {
            return Err(TwineSequencerError::Other(format!(
                "Empty result from bridge contract for batch {}",
                batch_number
            )));
        }

        let decoded = TwineChain::committedBatchCall::abi_decode_returns(&result).map_err(|e| {
            TwineSequencerError::Other(format!(
                "ABI decode failed for batch {}: {}",
                batch_number, e
            ))
        })?;

        Ok(FixedBytes::<32>::from(decoded.0))
    }
}
