//! client to aggregator service

use reqwest::Client;
use serde_json::json;
use twine_proof_scheduler_common::error::ProofSchedulerError;
use twine_types::proofs::ZkProof;

/// aggregator client
#[derive(Debug, Clone)]
pub struct AggregatorClient {
    /// aggregator url
    aggregator_url: String,
}

impl AggregatorClient {
    /// Creates new instance of aggregator client
    pub fn new(aggregator_url: String) -> Self { Self { aggregator_url } }

    /// send proof to aggregator
    pub async fn send_proof_to_aggregator(
        &self,
        proof: ZkProof,
    ) -> Result<(), ProofSchedulerError> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "twgg_sendProof",
            "params": [
                {
                    "type": "SP1",
                    "identifier": "scheduler",
                    "kind": "ExecutionProof",
                    "proof": proof
                }
                ],
            "id": 1
        });

        let client = Client::new();

        match client
            .post(&self.aggregator_url)
            .json(&payload)
            .send()
            .await
        {
            Ok(res) => {
                if !res.status().is_success() {
                    log::error!("could not send proof to the aggregator");
                    return Err(ProofSchedulerError::Other(
                        "could not send proof to aggregator".to_string(),
                    ));
                }
                log::info!("proof sent to the aggregator");
                Ok(())
            }
            Err(e) => {
                log::error!("could not send proof to the aggregator");
                Err(ProofSchedulerError::Other(format!(
                    "could not send proof to aggregator: {e}"
                )))
            }
        }
    }
}
