use std::ops::RangeInclusive;

use alloy_primitives::hex::FromHex;
use alloy_primitives::B256;
use eyre::{eyre, Result};
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use twine_types::BatchMeta;

use crate::TwineBatchApiClient;

/// Twine Batch Client
#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub struct BatchClient {
    inner: HttpClient,
}

#[allow(dead_code)]
impl BatchClient {
    /// Initialize new batch client
    pub fn new(url: &str) -> Self {
        let rpc_client = HttpClientBuilder::default().build(url).unwrap();
        Self { inner: rpc_client }
    }

    /// Query the highest **sealed** batch number currently known to the node.
    pub async fn get_latest_batch(&self) -> Result<u64> {
        self.inner.get_latest_batch().await.map_err(Into::into)
    }

    /// Fetch the complete [`BatchMeta`] object for the given `batch` number.
    pub async fn get_full_batch(&self, batch: u64, hydrate: Option<bool>) -> Result<BatchMeta> {
        self.inner
            .get_full_batch(batch, hydrate)
            .await
            .map_err(Into::into)
    }

    /// Retrieve the batch hash of the specified batch.
    pub async fn get_batch_hash(&self, batch: u64) -> Result<[u8; 32]> {
        let hash_hex = self
            .inner
            .get_batch_hash(batch)
            .await?
            .ok_or_else(|| eyre!("batch {batch} not found"))?;

        <[u8; 32]>::from_hex(hash_hex.trim_start_matches("0x"))
            .map_err(|_| eyre!("invalid batch-hash hex"))
    }

    /// Reverse lookup: find the **batch number** that owns the provided
    /// `batch_hash`.
    pub async fn get_batch_number(&self, batch_hash: B256) -> Result<u64> {
        let batch_number = self.inner.get_batch_number(batch_hash).await?;
        Ok(batch_number)
    }

    /// Map a **block number** to the **batch number** that contains it.
    pub async fn get_batch_number_for_block(&self, block: u64) -> Result<u64> {
        self.inner
            .get_batch_number_for_block(block)
            .await?
            .ok_or_else(|| eyre!("Block {block} not found"))
    }

    /// Return the inclusive range of **block numbers** that belong to the
    /// specified `batch`.
    pub async fn get_blocks_in_batch(&self, batch: u64) -> Result<RangeInclusive<u64>> {
        self.inner
            .get_blocks_in_batch(batch)
            .await?
            .ok_or_else(|| eyre!("Batch {batch} not found"))
    }
}
