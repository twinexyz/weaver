use std::sync::Arc;

use async_trait::async_trait;
use orchestrator_rs::config::Config;
use orchestrator_rs::worker::worker_manager::{WorkerManager, WorkerManagerResult};
use thiserror::Error;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;

use crate::request::EthereumWriterTransformAttempt;

pub struct EthereumWriterWorkerMgr<CFG> {
    _marker: std::marker::PhantomData<CFG>,
}

#[derive(Debug, Clone, Error)]
pub enum EthereumWriterWorkerMgrError {
    #[error("Worker Manager error: {0}")]
    GeneralError(String),
}

#[async_trait]
impl<CFG> WorkerManager for EthereumWriterWorkerMgr<CFG>
where
    CFG: Config<KeyType = String, ValueType = Vec<u8>> + Send + Sync + 'static,
{
    type Config = CFG;
    type TransformAttempt = EthereumWriterTransformAttempt;
    type WorkerManagerError = EthereumWriterWorkerMgrError;

    async fn new(
        init_config: Arc<Mutex<Self::Config>>,
        recv_channel: Receiver<Self::TransformAttempt>,
        send_channel: Sender<WorkerManagerResult<Self::TransformAttempt>>,
    ) -> Result<Self, Self::WorkerManagerError>
    where
        Self: Sized, {
        Ok(Self {
            _marker: std::marker::PhantomData,
        })
    }

    async fn wm_loop(&mut self) -> Result<(), Self::WorkerManagerError> {
        // Implement your worker manager loop logic here.
        // For now, just return Ok(()) to satisfy the return type.
        Ok(())
    }
}
