//! DA Layer Data source {Twine Chain}

use alloy_rlp::Encodable;
use twine_aggregator_common::TwineQuery;
use twine_common::retry::{retry_with_metrics, RetryConfig};
use twine_l1_eth::twine_l1_eth_reader::{EthReader, EthReaderBuilder};
use twine_rpc::client::BatchClient;

#[derive(Debug, Clone)]
#[allow(missing_docs)]
pub struct TwineReader {
    /// Twine Provider
    pub reader: EthReader,
    /// Twine Batch Client
    pub batch_client: BatchClient,
    /// Chain Id
    pub chain_id: u64,
}

impl TwineReader {
    /// Initialize a twine reader instance
    pub async fn new(rpc_url: &str, chain_id: u64) -> eyre::Result<Self> {
        let reader_builder = EthReaderBuilder::new().with_execution_rpc(rpc_url);
        let reader = reader_builder.build_with_chain_id(chain_id).await?;
        let batch_client = BatchClient::new(rpc_url);
        Ok(Self {
            reader,
            batch_client,
            chain_id,
        })
    }
}

#[async_trait::async_trait]
impl TwineQuery for TwineReader {
    async fn da_payload(&self, batch_id: u64) -> eyre::Result<Option<Vec<u8>>> {
        let config = RetryConfig::debug_default();
        let blocks_in_batch =
            retry_with_metrics(self.chain_id, "twine_getBlocksInBatch", &config, || async {
                self.batch_client.get_blocks_in_batch(batch_id).await
            })
            .await?;
        let mut batch_payload = Vec::new();

        for block in blocks_in_batch {
            let block = self
                .reader
                .execution
                .as_ref()
                .unwrap()
                .get_block(block.into(), true)
                .await?;
            if let Some(blk) = block {
                let mut out = Vec::new();
                blk.header.into_consensus().encode(&mut out);
                batch_payload.push(out);
            } else {
                return Ok(None);
            }
        }

        let mut serialized_batch = Vec::new();
        batch_payload.encode(&mut serialized_batch);

        Ok(Some(serialized_batch))
    }
}
