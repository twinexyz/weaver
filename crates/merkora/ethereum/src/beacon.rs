use std::time::Duration;

use reqwest::Client;
use tokio::time::sleep;
use twine_tcp_lib::eth::EthBeaconBlock;

#[derive(Debug, Clone)]
pub struct BeaconProvider {
    url: String,
    client: Client,
}

impl BeaconProvider {
    pub fn new(url: String) -> Self {
        let client = Client::new();
        Self { url, client }
    }

    pub async fn get_block_by_number(&self, block_number: u64) -> Option<EthBeaconBlock> {
        let url = format!("{}/eth/v2/beacon/blocks/{}", self.url, block_number);
        let mut retries = 0;
        let max_retries = 10;

        while retries < max_retries {
            let response = self.client.get(&url).send().await;
            match response {
                Ok(r) =>
                    if let Ok(res) = r.json::<serde_json::Value>().await {
                        if let Ok(block) = serde_json::from_value::<EthBeaconBlock>(res) {
                            return Some(block);
                        }
                    },
                Err(error) => {
                    tracing::error!(?error, "Failed querying beacon block");
                }
            }
            retries += 1;
            sleep(Duration::from_secs(2)).await;
        }

        None
    }
}
