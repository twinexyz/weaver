//! Consume attempt structure

use orchestrator_rs::consumer::ConsumeAttempt;
use serde::{Deserialize, Serialize};
use twine_proof_scheduler_common::error::ProofSchedulerError;

use crate::message_transform::message_transform_attempt::{
    SolanaMessageTransformAttemptID, SolanaMessageTransformReturnType,
};
use crate::message_transform::message_transform_request::SolanaMessageTransformRequestID;

/// uniquely identifies teh Consume Attempts
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct SolanaMessageConsumeAttemptID {
    /// sequentially increasing consume attempt identifier
    pub identifier: u64,
    /// transform attempt associated to the consume attempt
    pub transform_attempt_identifier: SolanaMessageTransformAttemptID,
}

impl From<SolanaMessageConsumeAttemptID> for SolanaMessageTransformAttemptID {
    fn from(value: SolanaMessageConsumeAttemptID) -> Self { value.transform_attempt_identifier }
}

impl From<SolanaMessageConsumeAttemptID> for SolanaMessageTransformRequestID {
    fn from(value: SolanaMessageConsumeAttemptID) -> Self {
        value.transform_attempt_identifier.into()
    }
}

/// Result consume context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaMessageTransformResultConsumeContext {}

/// Result consume value
#[derive(Debug, Clone)]
pub struct SolanaMessageTransformResultConsumeValue {}

/// Return context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaMessageTransformResultConsumeReturnContext {
    /// consume context
    pub consume_context: SolanaMessageTransformResultConsumeContext,
    /// consume value for the consumer
    pub consume_value: SolanaMessageTransformReturnType,
    /// consumption status
    pub consume_status: bool,
}

/// structure that represents the consume attempts of a `WorkerManagerResult`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaMessageConsumeAttempt {
    /// consume attempt ID
    pub identifier: SolanaMessageConsumeAttemptID,
    /// consume context
    pub consume_context: SolanaMessageTransformResultConsumeContext,
    /// consume value
    pub consume_value: SolanaMessageTransformReturnType, // TODO: fix redundant struct
    /// consumption status
    pub consume_status: bool,
}

impl ConsumeAttempt for SolanaMessageConsumeAttempt {
    type ConsumeCtx = SolanaMessageTransformResultConsumeContext;
    type ConsumeError = ProofSchedulerError;
    type ConsumeVal = SolanaMessageTransformReturnType;
    type Identifier = SolanaMessageConsumeAttemptID;
    type ReturnCtx = SolanaMessageTransformResultConsumeReturnContext;
    type TransformAttemptIdentifier = SolanaMessageTransformAttemptID;
    type TransformRequestIdentifier = SolanaMessageTransformRequestID;

    fn request_id(&self) -> Self::TransformRequestIdentifier { self.identifier.clone().into() }

    fn attempt_id(&self) -> Self::TransformAttemptIdentifier { self.identifier.clone().into() }

    fn consume_id(&self) -> Self::Identifier { self.identifier.clone() }

    fn set_return_context(&mut self, ctx: Self::ReturnCtx) {
        self.consume_context = ctx.consume_context;
        self.consume_value = ctx.consume_value;
        self.consume_status = ctx.consume_status;
    }

    /// Given the `ConsumeAttempt` is the latest in the stream,
    /// what dynamic configs need to be updated, Key is always a
    /// string, value is always a `Vec<u8>` representing the serialized
    /// value.
    fn get_dyn_configs(&self) -> Vec<(String, Vec<u8>)> { vec![] }
}

impl SolanaMessageConsumeAttempt {
    /// Creates new consume attempt
    pub fn new(
        identifier: SolanaMessageConsumeAttemptID,
        consume_context: SolanaMessageTransformResultConsumeContext,
        consume_value: SolanaMessageTransformReturnType,
    ) -> Self {
        Self {
            identifier,
            consume_context,
            consume_value,
            consume_status: false,
        }
    }
}
