//! JSON-RPC client for Ethereum execution layer queries

use alloy_eips::BlockId;
use alloy_primitives::Address;
use alloy_provider::{DynProvider, Provider, ProviderBuilder};
use alloy_rpc_types::{Block, Filter, Log, TransactionReceipt, TransactionRequest};
use eyre::{eyre, Context, Result};
use reth_tracing::tracing;
use twine_common::retry::{retry_with_metrics, RetryConfig};

/// Client for querying Ethereum execution layer data
#[derive(Debug, Clone)]
pub struct EthQueryExecutionClient {
    /// Eth execution provider
    provider: DynProvider,
    /// Chain Id
    chain_id: u64,
}

impl EthQueryExecutionClient {
    /// Creates new client with given RPC endpoint
    pub async fn new(rpc_url: &str) -> Result<Self> {
        let execution_provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);
        let chain_id = execution_provider.get_chain_id().await?;
        Ok(Self {
            provider: DynProvider::new(execution_provider),
            chain_id,
        })
    }

    /// Creates new client with given RPC endpoint and chain id
    pub fn new_with_chain_id(rpc_url: &str, chain_id: u64) -> Result<Self> {
        let execution_provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);
        Ok(Self {
            provider: DynProvider::new(execution_provider),
            chain_id,
        })
    }

    /// Gets current block number with retry logic
    pub async fn get_block_number(&self) -> Result<u64> {
        let config = RetryConfig::debug_default();
        let block_number =
            retry_with_metrics(self.chain_id, "eth_getBlockNumber", &config, || async {
                self.provider.get_block_number().await
            })
            .await?;
        Ok(block_number)
    }

    /// Gets block transactions with retry logic
    pub async fn get_block(&self, block: BlockId, full: bool) -> Result<Option<Block>> {
        let config = RetryConfig::debug_default();
        let block_data =
            retry_with_metrics(self.chain_id, "eth_getBlockByNumber", &config, || async {
                if full {
                    self.provider.get_block(block).full().await
                } else {
                    self.provider.get_block(block).hashes().await
                }
            })
            .await?;
        Ok(block_data)
    }

    /// Queries logs with filter and retry logic
    pub async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>> {
        let filter = filter.clone();
        let config = RetryConfig::debug_default();
        let logs = retry_with_metrics(self.chain_id, "eth_getLogs", &config, || async {
            self.provider.get_logs(&filter).await
        })
        .await
        .context("Failed getting logs")?;
        Ok(logs)
    }

    /// Gets transaction receipt with retry logic
    pub async fn get_tx_receipt(
        &self,
        tx_hash: alloy_primitives::TxHash,
    ) -> Result<Option<alloy_rpc_types::TransactionReceipt>> {
        let config = RetryConfig::debug_default();
        let receipt = retry_with_metrics(
            self.chain_id,
            "eth_getTransactionReceipt",
            &config,
            || async { self.provider.get_transaction_receipt(tx_hash).await },
        )
        .await
        .context("Failed getting tx receipts")?;
        Ok(receipt)
    }

    /// Gets all transaction receipts for a specific block with retry logic
    pub async fn get_block_receipts(
        &self,
        block: BlockId,
    ) -> Result<Option<Vec<TransactionReceipt>>> {
        let config = RetryConfig::debug_default();
        let receipts =
            retry_with_metrics(self.chain_id, "eth_getBlockReceipts", &config, || async {
                self.provider.get_block_receipts(block).await
            })
            .await
            .context("Failed to get block receipts after retries")?;
        Ok(receipts)
    }

    /// Gets nonce with retry logic
    pub async fn get_nonce(&self, address: Address) -> Result<u64> {
        let config = RetryConfig::debug_default();
        let nonce = retry_with_metrics(
            self.chain_id,
            "eth_getTransactionCount",
            &config,
            || async { self.provider.get_transaction_count(address).await },
        )
        .await?;
        Ok(nonce)
    }

    /// Gets fee estimation with retry logic
    pub async fn get_fee_estimation(&self) -> Result<(u128, u128)> {
        let config = RetryConfig::debug_default();
        let fee_estimation =
            retry_with_metrics(self.chain_id, "eth_feeHistory", &config, || async {
                self.provider.estimate_eip1559_fees().await
            })
            .await?;
        Ok((
            fee_estimation.max_fee_per_gas,
            fee_estimation.max_priority_fee_per_gas,
        ))
    }

    /// Gets gas estimation with retry logic
    pub async fn get_gas_estimation(&self, tx: &TransactionRequest) -> Result<u64> {
        let config = RetryConfig::debug_default();
        let gas_estimation =
            retry_with_metrics(self.chain_id, "eth_estimateGas", &config, || async {
                self.provider.estimate_gas(tx.clone()).await
            })
            .await?;
        Ok(gas_estimation)
    }
}

impl EthQueryExecutionClient {
    /// Fetch event
    #[allow(dead_code)]
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

// Helper function to sort logs
fn sort_logs(mut logs: Vec<Log>) -> Vec<Log> {
    logs.sort_by(|a, b| {
        a.block_number
            .unwrap_or_default()
            .cmp(&b.block_number.unwrap_or_default())
            .then_with(|| {
                a.log_index
                    .unwrap_or_default()
                    .cmp(&b.log_index.unwrap_or_default())
            })
    });
    logs
}
