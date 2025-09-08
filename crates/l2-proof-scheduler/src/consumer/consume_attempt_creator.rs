//! creates consume attempts for worker manager result

use std::collections::HashMap;
use std::time::{self, Duration};

use async_trait::async_trait;
use orchestrator_rs::config::Config;
use orchestrator_rs::consumer::{ConsumeAttempt, ConsumeAttemptCreator};
use twine_proof_scheduler_common::config::ProofSchedulerConfig;
use twine_proof_scheduler_common::error::ProofSchedulerError;

use crate::batch_transform::transform_attempt::{
    TwineBatchTransformAttempt, TwineBatchTransformAttemptID, TwineBatchTransformReturnType,
};
use crate::consumer::consume_attempt::{
    TwineBatchTransformResultConsumeAttempt, TwineBatchTransformResultConsumeAttemptID,
    TwineBatchTransformResultConsumeContext,
};

/// Max consume attempts per transform attempts
pub const DEFAULT_MAX_CONSUME_ATTEMPTS_PER_ATTEMPTS: u64 = 10;

/// consume attempt creator
#[derive(Debug)]
pub struct TwineBatchTransformResultConsumeAttemptCreator {
    /// maximum attempts per request, after this is reached, no new reattempts
    /// will be made
    max_attempts_per_request: u64,
    /// stores the latest transform attempts for each requests
    attempts: HashMap<TwineBatchTransformAttemptID, AttemptDetails>,
}

/// structure that holds attempts and creation time
#[derive(Debug)]
pub struct AttemptDetails {
    /// attempts
    attempt: TwineBatchTransformResultConsumeAttempt,
    /// creation time
    time: time::Instant,
}

#[async_trait]
impl ConsumeAttemptCreator for TwineBatchTransformResultConsumeAttemptCreator {
    type Config = ProofSchedulerConfig;
    type ConsumeAttempt = TwineBatchTransformResultConsumeAttempt;
    type ConsumeAttemptCreationError = ProofSchedulerError;
    type Output = TwineBatchTransformReturnType;
    type TransformAttempt = TwineBatchTransformAttempt;

    async fn new(config: std::sync::Arc<tokio::sync::Mutex<Self::Config>>) -> Self
    // constrain config
    where
        Self: Sized, {
        let mut max_attempts_per_request = DEFAULT_MAX_CONSUME_ATTEMPTS_PER_ATTEMPTS;

        if let Ok(max_attempts_from_config) = config
            .lock()
            .await
            .get("attempt.max_consume_attempts_per_attempts".to_string())
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

    /// Converts a `TransformAttempt` into a `ConsumeAttempt`.
    async fn create_new_attempt(
        &mut self,
        consume_attempt_id: Option<<Self::ConsumeAttempt as ConsumeAttempt>::Identifier>,
        request: &Self::TransformAttempt,
    ) -> Result<Self::ConsumeAttempt, Self::ConsumeAttemptCreationError> {
        if let Some(_) = self.attempts.get(&request.identifier) {
            return Err(ProofSchedulerError::KeyAlreadyExists(format!(
                "{:?}",
                request.identifier
            )));
        }

        let consume_attempt_id =
            consume_attempt_id.unwrap_or(TwineBatchTransformResultConsumeAttemptID {
                identifier: 0,
                transform_attempt_identifier: request.identifier.clone(),
            });
        let consume_attempt = TwineBatchTransformResultConsumeAttempt::new(
            consume_attempt_id,
            TwineBatchTransformResultConsumeContext {},
            request.return_type.clone().unwrap(), /* can unwrap here because this field could
                                                   * never be null */
        );

        let attempt_details = AttemptDetails {
            attempt: consume_attempt.clone(),
            time: time::Instant::now(),
        };

        self.attempts
            .insert(request.identifier.clone(), attempt_details);

        Ok(consume_attempt)
    }

    async fn create_new_reattempt(
        &mut self,
        attempt_id: <Self::ConsumeAttempt as ConsumeAttempt>::Identifier,
        error: <Self::ConsumeAttempt as ConsumeAttempt>::ReturnCtx,
    ) -> Result<Self::ConsumeAttempt, Self::ConsumeAttemptCreationError> {
        if let Some(attempt) = self
            .attempts
            .get_mut(&attempt_id.transform_attempt_identifier)
        {
            let mut new_identifier = attempt.attempt.identifier.clone();
            if new_identifier.identifier >= self.max_attempts_per_request {
                return Err(ProofSchedulerError::MaxReattemtsReached(format!(
                    "{:?}",
                    attempt_id.transform_attempt_identifier.clone()
                )));
            }
            new_identifier.identifier += 1;
            let transform_attempt = TwineBatchTransformResultConsumeAttempt::new(
                new_identifier,
                error.consume_context,
                error.consume_value,
            );

            let attempt_details = AttemptDetails {
                attempt: transform_attempt,
                time: time::Instant::now(),
            };

            *attempt = attempt_details
        }

        return Err(ProofSchedulerError::KeyNotFound(format!(
            "{:?}",
            attempt_id.transform_attempt_identifier
        )));
    }

    async fn prune_attempts(
        &mut self,
        attempt_id: <Self::ConsumeAttempt as ConsumeAttempt>::Identifier,
    ) -> Result<Duration, ProofSchedulerError> {
        let transform_request_id: TwineBatchTransformAttemptID = attempt_id.into();
        if let Some(attempts) = self.attempts.remove_entry(&transform_request_id) {
            log::info!("Removed {:?} from consume attempts record", attempts.0);
            let elapsed_time = attempts.1.time.elapsed().as_secs();
            return Ok(Duration::from_secs(elapsed_time));
        }

        log::warn!(
            "Key {:?} not found in consume attempts record",
            transform_request_id
        );
        Ok(Duration::from_secs(0))
    }
}
