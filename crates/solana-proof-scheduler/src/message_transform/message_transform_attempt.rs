//! Represents attempts made to convert the transform request
//! to output
use async_trait::async_trait;
use orchestrator_rs::transform::TransformAttempt;
use serde::{Deserialize, Serialize};
use twine_proof_scheduler_common::error::ProofSchedulerError;
use twine_types::proofs::ZkProof;

use crate::message_transform::message_transform_request::{
    SolanaMessageTransformCallCtx, SolanaMessageTransformInput, SolanaMessageTransformRequestID,
};

/// Uniquely identifies the transform attempts
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SolanaMessageTransformAttemptID {
    /// sequntial attempt identifier
    pub identifier: u64,
    /// represents the transform request associated with each
    /// transform attempts
    pub transform_request_id: SolanaMessageTransformRequestID,
}

impl SolanaMessageTransformAttemptID {
    /// creates new transform attempt id
    pub fn new(identifier: u64, transform_request_id: SolanaMessageTransformRequestID) -> Self {
        Self {
            identifier,
            transform_request_id,
        }
    }
}

impl From<SolanaMessageTransformAttemptID> for SolanaMessageTransformRequestID {
    fn from(value: SolanaMessageTransformAttemptID) -> Self { value.transform_request_id }
}

/// represents the attempts made to convert the transform request
/// to desired output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaMessageTransformAttempt {
    /// identifier to identify attempts
    pub identifier: SolanaMessageTransformAttemptID,
    /// context sent to the workers to convert the attempt to output
    pub call_ctx: SolanaMessageTransformCallCtx,
    /// input to convert into output using the call context by the workers
    pub call_val: SolanaMessageTransformInput,
    /// return value after the conversion by the workers
    pub return_type: Option<SolanaMessageTransformReturnType>,
}

/// return type from the workers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaMessageTransformReturnType(pub ZkProof);

/// return from the worker instances
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaMessageTransformReturnCtx {
    /// any additional data
    pub extra_data: Vec<u8>,
    /// batch transform input which was used to produce the return
    /// package
    pub call_type: SolanaMessageTransformInput,
    /// call context
    pub call_context: SolanaMessageTransformCallCtx,
}

#[async_trait]
impl TransformAttempt for SolanaMessageTransformAttempt {
    type CallArgsType = SolanaMessageTransformInput;
    type CallCtx = SolanaMessageTransformCallCtx;
    type Identifier = SolanaMessageTransformAttemptID;
    type ReturnCtx = SolanaMessageTransformReturnCtx;
    type ReturnPackage = (
        Self::Identifier,
        Self::ReturnCtx,
        Result<Self::ReturnType, Self::TransformError>,
    );
    type ReturnType = SolanaMessageTransformReturnType;
    type SendPackage = (Self::Identifier, Self::CallCtx, Self::CallArgsType);
    type TransformError = ProofSchedulerError;
    type TransformRequestIdentifier = SolanaMessageTransformRequestID;

    fn request_id(&self) -> Self::TransformRequestIdentifier { self.identifier.clone().into() }

    fn attempt_id(&self) -> Self::Identifier { self.identifier.clone() }

    fn new(
        attempt_id: Self::Identifier,
        call_ctx: Self::CallCtx,
        call_val: Self::CallArgsType,
    ) -> Self {
        Self {
            identifier: attempt_id,
            call_ctx,
            call_val,
            return_type: None,
        }
    }

    fn set_return_package(&mut self, return_pkg: Self::ReturnPackage) {
        if self.identifier != return_pkg.0 {
            return;
        }
        self.call_ctx = return_pkg.1.call_context;
        self.call_val = return_pkg.1.call_type;
        self.return_type = return_pkg.2.ok();
    }

    fn from_return_package(
        // TODO: redundant value
        attempt_id: Self::Identifier,
        return_package: Self::ReturnPackage,
    ) -> Self {
        Self {
            identifier: attempt_id,
            call_ctx: return_package.1.call_context,
            call_val: return_package.1.call_type,
            return_type: return_package.2.ok(),
        }
    }
}
