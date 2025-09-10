//! message transform request
use orchestrator_rs::transform::TransformRequest;
use serde::{Deserialize, Serialize};
use {serde_json, toml};

use crate::message_transform::message_transform_attempt::SolanaMessageTransformReturnType;

/// Unique Identifier that associates every transform request
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct SolanaMessageTransformRequestID {
    /// Sequential Identifier for Transform Request
    pub identifier: u64,
}

/// TransformRequestInput
/// Converts this transform request input to output by doing
/// operations on the input
/// eg. In this case this input is taken by the worker and
/// execution proof for the batch is calculated
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaMessageTransformInput {
    /// solana message event
    pub solana_message_event: SolanaEvent,
}

/// Transform request output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaMessageTransformOutput {}

/// Transform Request is the bundle of transform request id
/// and input which is converted to output by the workers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaMessageTransformRequest {
    /// unique id of request
    pub identifier: SolanaMessageTransformRequestID,
    /// transform input
    pub transform_input: SolanaMessageTransformInput,
    /// call context
    pub call_context: SolanaMessageTransformCallCtx,
}

/// additional information to be sent to the provers to produce
/// proofs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaMessageTransformCallCtx {
    /// devnet rpc
    pub solana_devnet_rpc: String,
}

/// solana message event
#[allow(missing_docs)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaEvent {
    pub chain_id: u64,
    pub nonce: u64,
    pub message_type: String,
    pub txn_hash: String,
    pub from_address: String,
    pub l1_token: String,
    pub l2_token: String,
    pub to_address: String,
    pub amount: String,
    pub block_number: u64,
    pub block_time: u64,
    pub data: Vec<u8>,
    pub prev_rolling_hash: Option<String>,
}

impl TransformRequest for SolanaMessageTransformRequest {
    type Identifier = SolanaMessageTransformRequestID;
    /// The type of the input for the transformation.
    type Input = SolanaMessageTransformInput;
    /// The type of the output expected after the transformation.
    type Output = SolanaMessageTransformReturnType;

    /// Returns the unique identifier for the transformation request.
    fn request_id(&self) -> Self::Identifier { self.identifier.clone() }

    /// Returns the input for the transformation request.
    fn input(&self) -> &Self::Input { &self.transform_input }

    /// Given the TransformRequest is the latest in the stream,
    /// what dynamic configs need to be updated, Key is always a
    /// string, value is always a `Vec<u8>` representing the serialized
    /// value
    fn get_dyn_configs(&self) -> Vec<(String, Vec<u8>)> {
        let next_message_nonce =
            toml::Value::Integer((self.transform_input.solana_message_event.nonce + 1) as i64);
        let next_message_nonce = serde_json::to_vec(&next_message_nonce).unwrap();

        let next_transfrom_request_id =
            toml::Value::Integer((self.identifier.identifier + 1) as i64);
        let next_transform_request_id = serde_json::to_vec(&next_transfrom_request_id).unwrap();
        vec![
            (
                "solana_emitter.next_message_nonce".to_string(),
                next_message_nonce,
            ),
            (
                "solana_emitter.next_transform_request_id".to_string(),
                next_transform_request_id,
            ),
        ]
    }
}
