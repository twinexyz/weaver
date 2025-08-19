//! Represents attempts made to convert the transform request
//! to output

use async_trait::async_trait;
use orchestrator_rs::transform::TransformAttempt;
use serde::{Deserialize, Serialize};

use crate::batch_transform::transform_request::{
    TwineBatchTransformInput, TwineBatchTransformRequestID,
};
use crate::error::TwineProofSchedulerError;

/// Uniquely identifies the transform attempts
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TwineBatchTransformAttemptID {
    /// sequntial attempt identifier
    pub identifier: u64,
    /// represents the transform request associated with each
    /// transform attempts
    pub transform_request_id: TwineBatchTransformRequestID,
}

impl TwineBatchTransformAttemptID {
    /// creates new transform attempt id
    pub fn new(identifier: u64, transform_request_id: TwineBatchTransformRequestID) -> Self {
        Self {
            identifier,
            transform_request_id,
        }
    }
}

impl From<TwineBatchTransformAttemptID> for TwineBatchTransformRequestID {
    fn from(value: TwineBatchTransformAttemptID) -> Self { value.transform_request_id }
}

/// represents the attempts made to convert the transform request
/// to desired output
#[derive(Debug, Clone)]
pub struct TwineBatchTransformAttempt {
    /// identifier to identify attempts
    pub identifier: TwineBatchTransformAttemptID,
    /// context sent to the workers to convert the attempt to output
    pub call_ctx: TwineBatchTransformCallCtx,
    /// input to convert into output using the call context by the workers
    pub call_val: TwineBatchTransformInput,
    /// return value after the conversion by the workers
    pub return_type: Option<TwineBatchTransformReturnType>,
}

/// call context sent alongside the input to the worker instance
#[derive(Debug, Clone)]
pub struct TwineBatchTransformCallCtx {
    /// rpc url to connect to twine node
    pub twine_node_rpc: String,
}

/// return from the worker instances
#[derive(Debug, Clone)]
pub struct TwineBatchTransformReturnCtx {
    /// any additional data
    pub extra_data: Vec<u8>,
    /// batch transform input which was used to produce the return
    /// package
    pub call_type: TwineBatchTransformInput,
    /// call context
    pub call_context: TwineBatchTransformCallCtx,
}

/// return type from the workers
#[derive(Debug, Clone)]
pub struct TwineBatchTransformReturnType(pub ZKProofBundle);

/// represents the zk proof structure that is returned by the worker instances
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZKProofBundle {
    /// version of the zk proof: it is associated with the verifying key
    pub version: u64, // TODO make it into an enum
    /// zk proof
    pub proof: Vec<u8>,
    /// zk public commitments
    pub public_value: Vec<u8>,
}

#[async_trait]
impl TransformAttempt for TwineBatchTransformAttempt {
    type CallArgsType = TwineBatchTransformInput;
    type CallCtx = TwineBatchTransformCallCtx;
    type Identifier = TwineBatchTransformAttemptID;
    type ReturnCtx = TwineBatchTransformReturnCtx;
    type ReturnPackage = (
        Self::Identifier,
        Self::ReturnCtx,
        Result<Self::ReturnType, Self::TransformError>,
    );
    type ReturnType = TwineBatchTransformReturnType;
    type SendPackage = (Self::Identifier, Self::CallCtx, Self::CallArgsType);
    type TransformError = TwineProofSchedulerError;
    type TransformRequestIdentifier = TwineBatchTransformRequestID;

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
