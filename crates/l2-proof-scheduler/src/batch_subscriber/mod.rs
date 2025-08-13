use async_trait::async_trait;
use orchestrator_rs::emitter::emitter::Emitter;
use thiserror::Error;

use crate::batch_transform_request::TwineBatchTransformRequest;
use crate::config::TwineProofSchedulerConfig;

#[derive(Debug, Clone)]
pub struct TwineBatchSubscriber {} // Orchestrator Emitter

#[derive(Debug, Clone, Error)]
pub enum TwineBatchSubscriberError {
    #[error("{0}")]
    Other(String),
}

#[async_trait]
impl Emitter for TwineBatchSubscriber {
    type Config = TwineProofSchedulerConfig;
    type Error = TwineBatchSubscriberError;
    type TransformRequest = TwineBatchTransformRequest;

    async fn new(
        init_config: Arc<Mutex<Self::Config>>,
        send_channel: Sender<Self::TransformRequest>,
        recv_channel: Receiver<EmissionState>,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized, {
        todo!()
    }

    /// The running loop of the emitter.
    /// This loop should run indefinitely, processing incoming requests in a
    /// sequential manner. See [`Emitter`] for more details.
    async fn emitter_loop(&mut self) -> Result<(), Self::Error> { todo!() }
}
