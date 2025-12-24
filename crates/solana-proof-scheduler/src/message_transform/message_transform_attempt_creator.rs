use std::collections::HashMap;
use std::time;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use log;
use orchestrator_rs::config::Config;
use orchestrator_rs::transform::{TransformAttempt, TransformAttemptCreator};
use twine_proof_scheduler_common::config::ProofSchedulerConfig;
use twine_proof_scheduler_common::error::ProofSchedulerError;

use crate::message_transform::message_transform_attempt::{
    SolanaMessageTransformAttempt, SolanaMessageTransformAttemptID,
    SolanaMessageTransformReturnType,
};
use crate::message_transform::message_transform_request::{
    SolanaMessageTransformInput, SolanaMessageTransformRequest, SolanaMessageTransformRequestID,
};

/// Maximum number of attempts for a request
pub const DEFAULT_MAX_ATTEMPTS_PER_REQUEST: u64 = 10;

/// Attempt Creator
#[derive(Debug, Clone)]
pub struct SolanaMessageTransformAttemptCreator {
    /// maximum attempts per request, after this is reached, no new reattempts
    /// will be made
    max_attempts_per_request: u64,
    /// stores the latest transform attempts for each requests
    attempts: HashMap<SolanaMessageTransformRequestID, AttemptDetails>,
}

/// Structure that hold attempts and its creation time
#[derive(Debug, Clone)]
pub struct AttemptDetails {
    /// attempts
    attempt: SolanaMessageTransformAttempt,
    /// time of creation
    time: time::Instant,
}

#[async_trait]
impl TransformAttemptCreator for SolanaMessageTransformAttemptCreator {
    type Config = ProofSchedulerConfig;
    type Input = SolanaMessageTransformInput;
    type Output = SolanaMessageTransformReturnType;
    type TransformAttempt = SolanaMessageTransformAttempt;
    type TransformAttemptCreationError = ProofSchedulerError;
    type TransformRequest = SolanaMessageTransformRequest;

    async fn new(config: std::sync::Arc<tokio::sync::Mutex<Self::Config>>) -> Self
    where
        Self: Sized, {
        let max_attempts_per_request = if let Ok(attempts) = config
            .lock()
            .await
            .get("attempt.max_attempts_per_request".to_string())
            .await
        {
            let max_attempts_from_config: toml::Value = serde_json::from_slice(&attempts)
                .expect("could not deserialize config into toml value");
            max_attempts_from_config.as_integer().unwrap_or(10) as u64
        } else {
            DEFAULT_MAX_ATTEMPTS_PER_REQUEST
        };

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
        if self.attempts.contains_key(&request.identifier) {
            return Err(ProofSchedulerError::KeyAlreadyExists(format!(
                "{:?}",
                request.identifier
            )));
        }
        let transform_attempt_id =
            transform_attempt_id.unwrap_or_else(|| SolanaMessageTransformAttemptID {
                identifier: 0,
                transform_request_id: request.identifier.clone(),
            });
        let transform_attempt = SolanaMessageTransformAttempt::new(
            transform_attempt_id,
            request.call_context.clone(),
            request.transform_input.clone(),
        );

        log::info!("new attempt for request {}", request.identifier.identifier);

        let attempt_details = AttemptDetails {
            attempt: transform_attempt.clone(),
            time: time::Instant::now(),
        };

        self.attempts
            .insert(request.identifier.clone(), attempt_details);

        Ok(transform_attempt)
    }

    /// Converts a `TransformRequest` into a `TransformAttempt`.
    async fn create_new_reattempt(
        &mut self,
        request: <Self::TransformAttempt as TransformAttempt>::Identifier,
        error: <Self::TransformAttempt as TransformAttempt>::ReturnPackage,
    ) -> Result<Self::TransformAttempt, Self::TransformAttemptCreationError> {
        if let Some(attempt) = self.attempts.get_mut(&request.transform_request_id) {
            let mut new_identifier = attempt.attempt.identifier.clone();
            if new_identifier.identifier >= self.max_attempts_per_request {
                return Err(ProofSchedulerError::MaxReattemtsReached(format!(
                    "{:?}",
                    request.transform_request_id.clone()
                )));
            }
            new_identifier.identifier += 1;
            let transform_attempt =
                SolanaMessageTransformAttempt::from_return_package(new_identifier, error);

            log::info!(
                "new transform reattempt for request {:?}",
                attempt.attempt.identifier.transform_request_id
            );

            let attempt_details = AttemptDetails {
                attempt: transform_attempt.clone(),
                time: Instant::now(),
            };

            *attempt = attempt_details;
            return Ok(transform_attempt);
        }

        return Err(ProofSchedulerError::KeyNotFound(format!(
            "{:?}",
            request.transform_request_id
        )));
    }

    async fn prune_attempts(
        &mut self,
        attempt_id: <Self::TransformAttempt as TransformAttempt>::Identifier,
    ) -> Result<Duration, ProofSchedulerError> {
        let transform_request_id: SolanaMessageTransformRequestID = attempt_id.into();
        if let Some(attempts) = self.attempts.remove_entry(&transform_request_id) {
            log::info!("Removed {:?} from transform attempts record", attempts.0);
            let elapsed_time = attempts.1.time.elapsed().as_secs();
            return Ok(Duration::from_secs(elapsed_time));
        }

        log::warn!("Key {transform_request_id:?} not found in transform attempts record");
        Ok(Duration::from_secs(0))
    }
}
