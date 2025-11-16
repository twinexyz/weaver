//! JSON RPC client for L2 chain communication

use std::time::Duration;

use alloy_primitives::FixedBytes;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::errors::TwineSequencerError;

/// JSON RPC request structure
#[derive(Debug, Serialize)]
pub struct JsonRpcRequest {
    /// JSON RPC protocol version
    pub jsonrpc: String,
    /// RPC method name
    pub method: String,
    /// Method parameters
    pub params: Value,
    /// Request identifier
    pub id: u64,
}

/// JSON RPC response structure
#[derive(Debug, Deserialize)]
pub struct JsonRpcResponse<T> {
    /// JSON RPC protocol version
    pub jsonrpc: String,
    /// Response result if successful
    pub result: Option<T>,
    /// Error details if request failed
    pub error: Option<JsonRpcError>,
    /// Request identifier
    pub id: u64,
}

/// JSON RPC error structure
#[derive(Debug, Deserialize)]
pub struct JsonRpcError {
    /// Error code
    pub code: i32,
    /// Human-readable error message
    pub message: String,
    /// Optional additional error data
    pub data: Option<Value>,
}

/// Batch hash response from L2 RPC
type BatchHashResponse = String;

/// L2 JSON RPC client
#[derive(Debug)]
pub struct L2RpcClient {
    /// HTTP client for JSON RPC calls
    client: Client,
    /// L2 RPC URL
    rpc_url: String,
}

impl L2RpcClient {
    /// Create a new L2 RPC client
    pub fn new(rpc_url: String) -> Result<Self, TwineSequencerError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| {
                TwineSequencerError::Other(format!("Failed to create HTTP client: {}", e))
            })?;

        Ok(Self { client, rpc_url })
    }

    /// Get batch hash for a specific batch number
    pub async fn get_batch_hash(
        &self,
        batch_number: u64,
    ) -> Result<FixedBytes<32>, TwineSequencerError> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "twine_getBatchHash".to_string(),
            params: json!([batch_number]),
            id: 1,
        };

        let response = self.send_request::<BatchHashResponse>(request).await?;
        match response {
            Some(hash_str) => {
                let hash_str = if hash_str.starts_with("0x") {
                    &hash_str[2..]
                } else {
                    &hash_str
                };

                let hash_bytes = hex::decode(hash_str).map_err(|e| {
                    TwineSequencerError::Other(format!("Invalid batch hash format: {}", e))
                })?;

                if hash_bytes.len() != 32 {
                    return Err(TwineSequencerError::Other(format!(
                        "Invalid batch hash length: expected 32 bytes, got {}",
                        hash_bytes.len()
                    )));
                }

                Ok(FixedBytes::from_slice(&hash_bytes))
            }
            None => Err(TwineSequencerError::Other(format!(
                "Batch {} not found",
                batch_number
            ))),
        }
    }

    /// Send a generic JSON RPC request to twine RPC service
    pub async fn send_request<T: for<'de> Deserialize<'de>>(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<T>, TwineSequencerError> {
        let response = self
            .client
            .post(&self.rpc_url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| TwineSequencerError::Other(format!("RPC request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(TwineSequencerError::Other(format!(
                "RPC request failed with status: {}",
                response.status()
            )));
        }

        let rpc_response: JsonRpcResponse<T> = response.json().await.map_err(|e| {
            TwineSequencerError::Other(format!("Failed to parse RPC response: {}", e))
        })?;

        if let Some(error) = rpc_response.error {
            return Err(TwineSequencerError::Other(format!(
                "RPC error {}: {}",
                error.code, error.message
            )));
        }

        Ok(rpc_response.result)
    }

    /// Get the RPC URL
    pub fn rpc_url(&self) -> &str { &self.rpc_url }
}
