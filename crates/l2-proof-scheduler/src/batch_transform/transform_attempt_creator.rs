//! creates transform attemtps for all transform requests
//! recreates transform attempts for failed transform attempts

use std::collections::HashMap;

use async_trait::async_trait;
use orchestrator_rs::config::Config;
use orchestrator_rs::transform::{TransformAttempt, TransformAttemptCreator};

use crate::batch_transform::transform_attempt::{
    TwineBatchTransformAttempt, TwineBatchTransformAttemptID, TwineBatchTransformReturnType,
};
use crate::batch_transform::transform_request::{
    TwineBatchTransformInput, TwineBatchTransformRequest, TwineBatchTransformRequestID,
};
use crate::error::TwineProofSchedulerError;

/// Transform attempt creator
#[derive(Debug, Clone)]
pub struct TwineBatchTransformAttemptCreator {
    /// maximum attempts per request, after this is reached, no new reattempts
    /// will be made
    max_attempts_per_request: u64,
    /// stores the latest transform attempts for each requests
    attempts: HashMap<TwineBatchTransformRequestID, TwineBatchTransformAttempt>, /* TODO: no need to store the full attempt */
}

#[async_trait]
impl TransformAttemptCreator for TwineBatchTransformAttemptCreator {
    type Input = TwineBatchTransformInput;
    type Output = TwineBatchTransformReturnType;
    type TransformAttempt = TwineBatchTransformAttempt;
    type TransformAttemptCreationError = TwineProofSchedulerError;
    type TransformRequest = TwineBatchTransformRequest;

    async fn new(_config: std::sync::Arc<tokio::sync::Mutex<impl Config>>) -> Self
    where
        Self: Sized, {
        Self {
            // config: _config,
            max_attempts_per_request: 10,
            attempts: HashMap::new(),
        } // TODO: constrain config
    }

    /// Converts a `TransformRequest` into a `TransformAttempt`.
    async fn create_new_attempt(
        &mut self,
        request: &Self::TransformRequest,
    ) -> Result<Self::TransformAttempt, Self::TransformAttemptCreationError> {
        if let Some(_) = self.attempts.get(&request.identifier) {
            return Err(TwineProofSchedulerError::KeyAlreadyExists(format!(
                "{:?}",
                request.identifier
            )));
        }
        let transform_attempt = TwineBatchTransformAttempt::new(
            TwineBatchTransformAttemptID::new(0, request.identifier.clone()),
            request.call_context.clone(),
            request.transform_input.clone(),
        );

        log::info!("new attempt for request {}", request.identifier.identifier);

        self.attempts
            .insert(request.identifier.clone(), transform_attempt.clone());

        Ok(transform_attempt)
    }

    /// Converts a `TransformRequest` into a `TransformAttempt`.
    async fn create_new_reattempt(
        &mut self,
        request: <Self::TransformAttempt as TransformAttempt>::Identifier,
        error: <Self::TransformAttempt as TransformAttempt>::ReturnPackage,
    ) -> Result<Self::TransformAttempt, Self::TransformAttemptCreationError> {
        if let Some(attempt) = self.attempts.get_mut(&request.transform_request_id) {
            let mut new_identifier = attempt.identifier.clone();
            if new_identifier.identifier >= self.max_attempts_per_request {
                return Err(TwineProofSchedulerError::MaxReattemtsReached(format!(
                    "{:?}",
                    request.transform_request_id.clone()
                )));
            }
            new_identifier.identifier += 1;
            let transform_attempt =
                TwineBatchTransformAttempt::from_return_package(new_identifier, error);
            *attempt = transform_attempt.clone();
            return Ok(transform_attempt);
        }

        return Err(TwineProofSchedulerError::KeyNotFound(format!(
            "{:?}",
            request.transform_request_id
        )));
    }
}
