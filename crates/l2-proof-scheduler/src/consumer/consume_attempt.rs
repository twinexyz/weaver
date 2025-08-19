//! consume attempt

use orchestrator_rs::consumer::ConsumeAttempt;

use crate::batch_transform::transform_attempt::{
    TwineBatchTransformAttemptID, TwineBatchTransformReturnType,
};
use crate::batch_transform::transform_request::TwineBatchTransformRequestID;
use crate::error::TwineProofSchedulerError;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
/// uniquely identifies teh Consume Attempts
pub struct TwineBatchTransformResultConsumeAttemptID {
    /// sequentially increasing consume attempt identifier
    pub identifier: u64,
    /// transform attempt associated to the consume attempt
    pub transform_attempt_identifier: TwineBatchTransformAttemptID,
}

impl TwineBatchTransformResultConsumeAttemptID {
    /// Creates new consume attempt id
    pub fn new(
        identifier: u64,
        transform_attempt_identifier: TwineBatchTransformAttemptID,
    ) -> Self {
        Self {
            identifier,
            transform_attempt_identifier,
        }
    }
}

impl From<TwineBatchTransformResultConsumeAttemptID> for TwineBatchTransformAttemptID {
    fn from(value: TwineBatchTransformResultConsumeAttemptID) -> Self {
        value.transform_attempt_identifier
    }
}

impl From<TwineBatchTransformResultConsumeAttemptID> for TwineBatchTransformRequestID {
    fn from(value: TwineBatchTransformResultConsumeAttemptID) -> Self {
        value.transform_attempt_identifier.into()
    }
}

/// Result consume context
#[derive(Debug, Clone)]
pub struct TwineBatchTransformResultConsumeContext {}

/// Result consume value
#[derive(Debug, Clone)]
pub struct TwineBatchTransformResultConsumeValue {}

/// Return context
#[derive(Debug, Clone)]
pub struct TwineBatchTransformResultConsumeReturnContext {
    /// consume context
    pub consume_context: TwineBatchTransformResultConsumeContext,
    /// consume value for the consumer
    pub consume_value: TwineBatchTransformReturnType,
    /// consumption status
    pub consume_status: bool,
}

/// structure that represents the consume attempts of a `WorkerManagerResult`
#[derive(Debug, Clone)]
pub struct TwineBatchTransformResultConsumeAttempt {
    /// consume attempt ID
    pub identifier: TwineBatchTransformResultConsumeAttemptID,
    /// consume context
    pub consume_context: TwineBatchTransformResultConsumeContext,
    /// consume value
    pub consume_value: TwineBatchTransformReturnType, // TODO: fix redundant struct
    /// consumption status
    pub consume_status: bool,
}

impl ConsumeAttempt for TwineBatchTransformResultConsumeAttempt {
    type ConsumeCtx = TwineBatchTransformResultConsumeContext;
    type ConsumeError = TwineProofSchedulerError;
    type ConsumeVal = TwineBatchTransformReturnType;
    type Identifier = TwineBatchTransformResultConsumeAttemptID;
    type ReturnCtx = TwineBatchTransformResultConsumeReturnContext;
    type TransformAttemptIdentifier = TwineBatchTransformAttemptID;
    type TransformRequestIdentifier = TwineBatchTransformRequestID;

    fn request_id(&self) -> Self::TransformRequestIdentifier { self.identifier.clone().into() }

    fn attempt_id(&self) -> Self::TransformAttemptIdentifier { self.identifier.clone().into() }

    fn consume_id(&self) -> Self::Identifier { self.identifier.clone() }

    fn set_return_context(&mut self, ctx: Self::ReturnCtx) {
        self.consume_context = ctx.consume_context;
        self.consume_value = ctx.consume_value;
        self.consume_status = ctx.consume_status;
    }

    /// Given the ConsumeAttempt is the latest in the stream,
    /// what dynamic configs need to be updated, Key is always a
    /// string, value is always a `Vec<u8>` representing the serialized
    /// value.
    fn get_dyn_configs(&self) -> Vec<(String, Vec<u8>)> { todo!() }
}

impl TwineBatchTransformResultConsumeAttempt {
    /// Creates new consume attempt
    pub fn new(
        identifier: TwineBatchTransformResultConsumeAttemptID,
        consume_context: TwineBatchTransformResultConsumeContext,
        consume_value: TwineBatchTransformReturnType,
    ) -> Self {
        Self {
            identifier,
            consume_context,
            consume_value,
            consume_status: false,
        }
    }
}
