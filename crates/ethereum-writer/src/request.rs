use std::fmt::Display;

use orchestrator_rs::transform::{TransformAttempt, TransformRequest};
use thiserror::Error;

use crate::types::{L1PostingParams, L1PostingParamsWithAuxData};

#[derive(Debug, Clone)]
pub struct EthereumWriterTransformRequest {
    pub identifier: [u8; 32],
    pub input: L1PostingParamsWithAuxData,
}

impl TransformRequest for EthereumWriterTransformRequest {
    type Identifier = [u8; 32];
    type Input = L1PostingParamsWithAuxData;
    type Output = L1PostingParams;

    fn get_dyn_configs(&self) -> Vec<(String, Vec<u8>)> { vec![] }

    fn input(&self) -> &Self::Input { &self.input }

    fn request_id(&self) -> Self::Identifier { self.identifier }
}

#[derive(Debug, Clone)]
pub struct EthereumWriterTransformAttempt {
    pub identifier: [u8; 32],
    pub input: L1PostingParamsWithAuxData,
    pub output: Option<L1PostingParams>,
}

#[derive(Debug, Error, Clone)]
pub enum TransformAttemptError {
    #[error("Transform attempt failed: {0}")]
    GeneralError(String),
}

impl TransformAttempt for EthereumWriterTransformAttempt {
    type CallArgsType = L1PostingParamsWithAuxData;
    type CallCtx = ();
    type Identifier = [u8; 32];
    type ReturnCtx = ();
    type ReturnType = L1PostingParams;
    type TransformError = TransformAttemptError;
    type TransformRequestIdentifier = [u8; 32];

    fn request_id(&self) -> Self::Identifier { self.identifier }

    fn attempt_id(&self) -> Self::Identifier { self.identifier }

    fn new(
        identifier: Self::Identifier,
        _call_ctx: Self::CallCtx,
        call_args: Self::CallArgsType,
    ) -> Self {
        Self {
            identifier,
            input: call_args,
            output: None,
        }
    }

    fn from_return_package(
        attempt_id: Self::Identifier,
        return_package: Self::ReturnPackage,
    ) -> Self {
        Self {
            identifier: attempt_id,
            input: Default::default(),
            output: Some(return_package.2.unwrap()),
        }
    }

    fn set_return_package(&mut self, return_pkg: Self::ReturnPackage) {
        self.identifier = return_pkg.0;
        self.output = Some(return_pkg.2.unwrap());
    }
}
