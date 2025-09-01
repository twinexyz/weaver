//! creates consume attempts for worker manager result

use std::collections::HashMap;

use async_trait::async_trait;
use orchestrator_rs::config::Config;
use orchestrator_rs::consumer::{ConsumeAttempt, ConsumeAttemptCreator};

use crate::batch_transform::transform_attempt::{
    TwineBatchTransformAttempt, TwineBatchTransformAttemptID, TwineBatchTransformReturnType,
};
use crate::config::TwineProofSchedulerConfig;
use crate::consumer::consume_attempt::{
    TwineBatchTransformResultConsumeAttempt, TwineBatchTransformResultConsumeAttemptID,
    TwineBatchTransformResultConsumeContext,
};
use crate::error::TwineProofSchedulerError;

/// Max consume attempts per transform attempts
pub const DEFAULT_MAX_CONSUME_ATTEMPTS_PER_ATTEMPTS: u64 = 10;

/// consume attempt creator
#[derive(Debug)]
pub struct TwineBatchTransformResultConsumeAttemptCreator {
    /// maximum attempts per request, after this is reached, no new reattempts
    /// will be made
    max_attempts_per_request: u64,
    /// stores the latest transform attempts for each requests
    attempts: HashMap<TwineBatchTransformAttemptID, TwineBatchTransformResultConsumeAttempt>,
}

#[async_trait]
impl ConsumeAttemptCreator for TwineBatchTransformResultConsumeAttemptCreator {
    type Config = TwineProofSchedulerConfig;
    type ConsumeAttempt = TwineBatchTransformResultConsumeAttempt;
    type ConsumeAttemptCreationError = TwineProofSchedulerError;
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
            return Err(TwineProofSchedulerError::KeyAlreadyExists(format!(
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

        self.attempts
            .insert(request.identifier.clone(), consume_attempt.clone());

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
            let mut new_identifier = attempt.identifier.clone();
            if new_identifier.identifier >= self.max_attempts_per_request {
                return Err(TwineProofSchedulerError::MaxReattemtsReached(format!(
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
            *attempt = transform_attempt
        }

        return Err(TwineProofSchedulerError::KeyNotFound(format!(
            "{:?}",
            attempt_id.transform_attempt_identifier
        )));
    }

    async fn prune_attempts(
        &mut self,
        attempt_id: <Self::ConsumeAttempt as ConsumeAttempt>::Identifier,
    ) -> Result<(), TwineProofSchedulerError> {
        let transform_request_id: TwineBatchTransformAttemptID = attempt_id.into();
        if let Some(attempts) = self.attempts.remove_entry(&transform_request_id) {
            log::info!("Removed {:?} from consume attempts record", attempts.0);
            return Ok(());
        }

        log::warn!(
            "Key {:?} not found in consume attempts record",
            transform_request_id
        );
        Ok(())
    }
}
