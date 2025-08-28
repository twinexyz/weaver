//! creates consume attempts for worker manager result

use std::collections::HashMap;

use async_trait::async_trait;
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

    async fn new(_config: std::sync::Arc<tokio::sync::Mutex<Self::Config>>) -> Self
    // constrain config
    where
        Self: Sized, {
        Self {
            max_attempts_per_request: 10,
            attempts: HashMap::new(),
        }
    }

    /// Converts a `TransformAttempt` into a `ConsumeAttempt`.
    async fn create_new_attempt(
        &mut self,
        request: &Self::TransformAttempt,
    ) -> Result<Self::ConsumeAttempt, Self::ConsumeAttemptCreationError> {
        if let Some(_) = self.attempts.get(&request.identifier) {
            return Err(TwineProofSchedulerError::KeyAlreadyExists(format!(
                "{:?}",
                request.identifier
            )));
        }
        let consume_attempt = TwineBatchTransformResultConsumeAttempt::new(
            TwineBatchTransformResultConsumeAttemptID::new(0, request.identifier.clone()),
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
}
