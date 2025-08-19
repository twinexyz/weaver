//! consumes worker manager result

use std::sync::Arc;

use async_trait::async_trait;
use orchestrator_rs::consumer::consumer::ConsumeAttemptResult;
use orchestrator_rs::consumer::Consumer;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;

use crate::config::TwineProofSchedulerConfig;
use crate::consumer::consume_attempt::TwineBatchTransformResultConsumeAttempt;
use crate::error::TwineProofSchedulerError;

/// Structure that represents the worker manager result consumer
#[derive(Debug)]
pub struct TwineBatchTransformResultConsumer {}

#[async_trait]
impl Consumer for TwineBatchTransformResultConsumer {
    type Config = TwineProofSchedulerConfig;
    type ConsumeAttempt = TwineBatchTransformResultConsumeAttempt;
    type ConsumeError = TwineProofSchedulerError;

    async fn new(
        _init_config: Arc<Mutex<Self::Config>>,
        _recv_channel: Receiver<Self::ConsumeAttempt>,
        _end_channel: Sender<ConsumeAttemptResult<Self::ConsumeAttempt>>,
    ) -> Result<Self, Self::ConsumeError>
    where
        Self: Sized, {
        todo!()
    }

    /// The running loop of the consumer.
    /// This loop should run indefinitely, processing incoming consume attempt
    /// requests in a sequential manner. See [`Consumer`] for more details.
    async fn consumer_loop(&mut self) -> Result<(), Self::ConsumeError> { todo!() }
}
