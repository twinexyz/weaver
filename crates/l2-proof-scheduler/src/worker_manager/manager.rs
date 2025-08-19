//! manages worker's connection

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use orchestrator_rs::config::Config;
use orchestrator_rs::worker::worker_manager::{WorkerManager, WorkerManagerResult};
use tokio::net::TcpListener;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::batch_transform::transform_attempt::TwineBatchTransformAttempt;
use crate::config::TwineProofSchedulerConfig;
use crate::error::TwineProofSchedulerError;
use crate::utils::to_bytes_u64;
use crate::worker_manager::connections::{ConnectionID, Connections};

/// Twine Worker Manager
#[derive(Debug)]
pub struct TwineWorkerManager {
    /// port where the manager exposes its wss server to
    /// the workers
    pub binding_port: u64,
    /// receives transform attempts from the processor
    pub transform_attempt_receiver: Receiver<TwineBatchTransformAttempt>,
    /// sends worker sent results to the consumer
    pub worker_result_sender: Sender<WorkerManagerResult<TwineBatchTransformAttempt>>,
}

#[async_trait]
impl WorkerManager for TwineWorkerManager {
    type Config = TwineProofSchedulerConfig;
    type TransformAttempt = TwineBatchTransformAttempt;
    type WorkerManagerError = TwineProofSchedulerError;

    async fn new(
        init_config: Arc<Mutex<Self::Config>>,
        recv_channel: Receiver<Self::TransformAttempt>,
        send_channel: Sender<WorkerManagerResult<Self::TransformAttempt>>,
    ) -> Result<Self, Self::WorkerManagerError>
    where
        Self: Sized, {
        let binding_port = init_config
            .lock()
            .await
            .get("worker_manager.binding_port".to_string())
            .await?;
        let binding_port =
            to_bytes_u64(&binding_port).map_err(|e| TwineProofSchedulerError::Other(e))?;
        let binding_port: u64 = u64::from_be_bytes(binding_port);

        Ok(Self {
            binding_port,
            transform_attempt_receiver: recv_channel,
            worker_result_sender: send_channel,
        })
    }

    async fn wm_loop(&mut self) -> Result<(), Self::WorkerManagerError> {
        let job_mutex = Arc::new(Mutex::new(None));
        let (mut wss_server_job, mut job_handle_job) = start_worker_register_server(
            self.binding_port,
            self.worker_result_sender.clone(),
            job_mutex.clone(),
        )
        .await?;

        'outer: loop {
            tokio::select! {
                Some(input) = self.transform_attempt_receiver.recv() => {
                    'inner: loop {
                        let mut job = job_mutex.lock().await;
                        match job.clone(){
                            Some(_) => continue 'inner,
                            None => {
                                *job = Some(input);
                                continue 'outer;
                            }
                        }
                }
                }

                 _ = &mut wss_server_job => {}
                 _ = &mut job_handle_job => {}
            }
        }
    }
}

/// starts a wss server to accept the incoming workers
///
/// ## Arguments
/// - `sender` - sender to send the worker produced results to the consumer
/// - `job_mutex` - mutex that holds the un-assigned job, when this job is
///   received by one of the workers, the job is consumed and new unassigned job
///   is added no two connections can receive the same job
pub async fn start_worker_register_server(
    bind_port: u64,
    sender: Sender<WorkerManagerResult<TwineBatchTransformAttempt>>,
    job_mutex: Arc<Mutex<Option<TwineBatchTransformAttempt>>>,
) -> Result<(JoinHandle<()>, JoinHandle<()>), TwineProofSchedulerError> {
    let address = format!("http://0.0.0.0:{}", bind_port);
    let listener = TcpListener::bind(&address)
        .await
        .map_err(|e| TwineProofSchedulerError::Other(format!("{e}")))?;

    log::info!("starting wss server on: {}", address);

    let connections = Arc::new(Connections {
        assigned_jobs: Mutex::new(HashMap::new()),
        connection_status: Mutex::new(HashMap::new()),
    });

    let cloned_connection = connections.clone();
    let cloned_sender = sender.clone();
    let wss_handle = tokio::spawn(async move {
        let mut total_connections = ConnectionID(0u64);
        loop {
            if let Ok((stream, _)) = listener.accept().await {
                let cloned_connection = cloned_connection.clone();
                let cloned_sender = cloned_sender.clone();
                let job_mtx = job_mutex.clone();
                let connection_id = ConnectionID(total_connections.0 + 1);
                tokio::spawn(async move {
                    cloned_connection
                        .accept_connection(connection_id, stream, cloned_sender, job_mtx)
                        .await
                });
                total_connections.0 += 1;
            }
        }
    });

    let cloned_connection = connections.clone();

    let job_handle_job = tokio::spawn(async move {
        cloned_connection.handle_assigned_jobs(sender).await;
    });
    Ok((wss_handle, job_handle_job))
}
