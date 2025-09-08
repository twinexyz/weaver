//! subscribes to the solana l1 for bridge messages

use std::sync::Arc;

use async_trait::async_trait;
use orchestrator_rs::emitter::emitter::{EmissionState, Emitter};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use twine_proof_scheduler_common::config::ProofSchedulerConfig;
use twine_proof_scheduler_common::error::ProofSchedulerError;

use crate::message_transform::message_transform_request::SolanaMessageTransformRequest;

/// Solana message subscriber
#[derive(Debug, Clone)]
pub struct SolanaMessageSubscriber {
    /// connection string to merkora's db
    _db_conn_string: String,
}

#[async_trait]
impl Emitter for SolanaMessageSubscriber {
    type Config = ProofSchedulerConfig;
    type Error = ProofSchedulerError;
    type TransformRequest = SolanaMessageTransformRequest;

    async fn new(
        _init_config: Arc<Mutex<Self::Config>>,
        _send_channel: Sender<Self::TransformRequest>,
        _recv_channel: Receiver<EmissionState>,
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
