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
use crate::config::TwineProofSchedulerConfig;
use crate::error::TwineProofSchedulerError;

/// Maximum number of attempts for a request
pub const DEFAULT_MAX_ATTEMPTS_PER_REQUEST: u64 = 10;

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
    type Config = TwineProofSchedulerConfig;
    type Input = TwineBatchTransformInput;
    type Output = TwineBatchTransformReturnType;
    type TransformAttempt = TwineBatchTransformAttempt;
    type TransformAttemptCreationError = TwineProofSchedulerError;
    type TransformRequest = TwineBatchTransformRequest;

    async fn new(config: std::sync::Arc<tokio::sync::Mutex<Self::Config>>) -> Self
    where
        Self: Sized, {
        let mut max_attempts_per_request = DEFAULT_MAX_ATTEMPTS_PER_REQUEST;

        if let Ok(max_attempts_from_config) = config
            .lock()
            .await
            .get("attempt.max_attempts_per_request".to_string())
            .await
        {
            let max_attempts_from_config: toml::Value =
                serde_json::from_slice(&max_attempts_from_config)
                    .expect("could not deserialize config into toml value");

            max_attempts_per_request = max_attempts_from_config.as_integer().unwrap_or(10) as u64;
        }

        Self {
            max_attempts_per_request,
            attempts: HashMap::new(),
        }
    }

    /// Converts a `TransformRequest` into a `TransformAttempt`.
    async fn create_new_attempt(
        &mut self,
        transform_attempt_id: Option<<Self::TransformAttempt as TransformAttempt>::Identifier>,
        request: &Self::TransformRequest,
    ) -> Result<Self::TransformAttempt, Self::TransformAttemptCreationError> {
        if let Some(_) = self.attempts.get(&request.identifier) {
            return Err(TwineProofSchedulerError::KeyAlreadyExists(format!(
                "{:?}",
                request.identifier
            )));
        }
        let transform_attempt_id = transform_attempt_id.unwrap_or(TwineBatchTransformAttemptID {
            identifier: 0,
            transform_request_id: request.identifier.clone(),
        });
        let transform_attempt = TwineBatchTransformAttempt::new(
            transform_attempt_id,
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

    async fn prune_attempts(
        &mut self,
        attempt_id: <Self::TransformAttempt as TransformAttempt>::Identifier,
    ) -> Result<(), TwineProofSchedulerError> {
        let transform_request_id: TwineBatchTransformRequestID = attempt_id.into();
        if let Some(attempts) = self.attempts.remove_entry(&transform_request_id) {
            log::info!("Removed {:?} from transform attempts record", attempts.0);
            return Ok(());
        }

        log::warn!(
            "Key {:?} not found in transform attempts record",
            transform_request_id
        );
        Ok(())
    }
}
