//! JSON-RPC client for Ethereum execution layer queries

use alloy_eips::BlockId;
use alloy_primitives::Address;
use alloy_provider::{DynProvider, Provider, ProviderBuilder};
use alloy_rpc_types::{Block, Filter, Log, TransactionReceipt};
use eyre::{eyre, Context, Result};
use twine_common::retry::retry_default_with_jitter;

use crate::clients::sort_logs;

/// Client for querying Ethereum execution layer data
#[derive(Debug, Clone)]
pub struct EthQueryExecutionClient {
    /// Eth execution provider
    provider: DynProvider,
}

impl EthQueryExecutionClient {
    /// Creates new client with given RPC endpoint
    pub fn new(rpc_url: &str) -> Result<Self> {
        let execution_provider = ProviderBuilder::new().on_http(rpc_url.parse()?);
        Ok(Self {
            provider: DynProvider::new(execution_provider),
        })
    }

    /// Gets current block number with retry logic
    pub async fn get_block_number(&self) -> Result<u64> {
        let op = || async { self.provider.get_block_number().await };
        Ok(retry_default_with_jitter(op).await?)
    }

    /// Gets block transactions with retry logic
    pub async fn get_block(&self, block: BlockId) -> Result<Option<Block>> {
        let op = || async { self.provider.get_block(block).await };
        Ok(retry_default_with_jitter(op).await?)
    }

    /// Queries logs with filter and retry logic
    pub async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>> {
        let filter = filter.clone();
        let op = || async { self.provider.get_logs(&filter).await };
        Ok(retry_default_with_jitter(op)
            .await
            .context("Failed getting logs")?)
    }

    /// Gets transaction receipt with retry logic
    pub async fn get_tx_receipt(
        &self,
        tx_hash: alloy_primitives::TxHash,
    ) -> Result<Option<alloy_rpc_types::TransactionReceipt>> {
        let op = || async { self.provider.get_transaction_receipt(tx_hash).await };
        Ok(retry_default_with_jitter(op)
            .await
            .context("Failed getting tx receipts")?)
    }

    /// Gets all transaction receipts for a specific block with retry logic
    pub async fn get_block_receipts(
        &self,
        block: BlockId,
    ) -> Result<Option<Vec<TransactionReceipt>>> {
        let op = || async { self.provider.get_block_receipts(block).await };

        Ok(retry_default_with_jitter(op)
            .await
            .context("Failed to get block receipts after retries")?)
    }
}

impl EthQueryExecutionClient {
    /// Fetch event
    pub(crate) async fn fetch_event_inner(
        &self,
        from_block: u64,
        to_block: u64,
        events: impl IntoIterator<Item = impl AsRef<[u8]>>,
        addresses: Vec<Address>,
    ) -> eyre::Result<Vec<Log>> {
        let addresses = addresses.clone();
        // let event: Vec<_> = events.into_iter().collect();
        let filter = Filter::new()
            .from_block(from_block)
            .to_block(to_block)
            .events(events)
            .address(addresses);

        match self.get_logs(&filter).await {
            Ok(logs) => Ok(sort_logs(logs)),
            Err(e) => {
                tracing::error!(err=?e, "Error fetching event");
                return Err(eyre!(""));
            }
        }
    }
}
