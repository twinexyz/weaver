//! RPC Queries for Ethereum Beacon Client

use eyre::Result;
use reqwest::Client;
use twine_common::retry::retry_default;
use twine_ethereum_consensus_prover_lib::eth::EthBeaconBlock;

/// Client for querying Ethereum beacon chain data
#[derive(Debug, Clone)]
pub struct EthQueryBeaconClient {
    url: String,
}

impl EthQueryBeaconClient {
    /// New ethereum beacon query client
    pub fn new(url: String) -> Self { Self { url } }

    /// Get ethereum beacon block
    pub async fn get_block_by_number(&self, block_number: u64) -> Result<EthBeaconBlock> {
        let url = format!("{}/eth/v2/beacon/blocks/{}", self.url, block_number);
        let client = Client::new();

        let operation = || async {
            let response = client.get(&url).send().await.map_err(|e| {
                tracing::error!(?e, "Request failed");
                e
            })?;

            response.json::<EthBeaconBlock>().await.map_err(|e| {
                tracing::error!(?e, "Failed to parse beacon block");
                e
            })
        };

        Ok(retry_default(operation).await?)
    }
}
