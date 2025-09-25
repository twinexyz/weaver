//! manages worker's connection

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use orchestrator_rs::config::Config;
use orchestrator_rs::worker::worker_manager::{WorkerManager, WorkerManagerResult};
use tokio::net::TcpListener;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use twine_proof_scheduler_common::config::ProofSchedulerConfig;
use twine_proof_scheduler_common::error::ProofSchedulerError;

use crate::batch_transform::transform_attempt::TwineBatchTransformAttempt;
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
    /// job completion timeout: removes the workers if their job timeout exceeds
    pub job_completion_timeout: u64,
}

#[async_trait]
impl WorkerManager for TwineWorkerManager {
    type Config = ProofSchedulerConfig;
    type TransformAttempt = TwineBatchTransformAttempt;
    type WorkerManagerError = ProofSchedulerError;

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

        let binding_port: toml::Value = serde_json::from_slice(&binding_port).unwrap();

        let binding_port = binding_port.as_integer().unwrap() as u64;

        let job_completion_timeout = init_config
            .lock()
            .await
            .get("worker_manager.binding_port".to_string())
            .await?;

        let job_completion_timeout: toml::Value =
            serde_json::from_slice(&job_completion_timeout).unwrap();

        let job_completion_timeout = job_completion_timeout.as_integer().unwrap() as u64;

        Ok(Self {
            binding_port,
            transform_attempt_receiver: recv_channel,
            worker_result_sender: send_channel,
            job_completion_timeout,
        })
    }

    async fn wm_loop(&mut self) -> Result<(), Self::WorkerManagerError> {
        log::info!("starting wm loop");
        let job_mutex = Arc::new(Mutex::new(None));
        let (wss_server_job, job_handle_job) = start_worker_register_server(
            self.binding_port,
            self.worker_result_sender.clone(),
            job_mutex.clone(),
            self.job_completion_timeout,
        )
        .await?;

        tokio::pin!(wss_server_job);
        tokio::pin!(job_handle_job);

        'outer: loop {
            tokio::select! {
                Some(input) = self.transform_attempt_receiver.recv() => {
                    'inner: loop {
                        sleep(Duration::from_secs(1)).await;
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
    job_completion_timeout: u64,
) -> Result<(JoinHandle<()>, JoinHandle<()>), ProofSchedulerError> {
    let address = format!("0.0.0.0:{bind_port}");
    let listener = TcpListener::bind(&address)
        .await
        .map_err(|e| ProofSchedulerError::Other(format!("{e}")))?;

    log::info!("starting wss server on: {address}");

    let connections = Arc::new(Connections {
        assigned_jobs: Mutex::new(HashMap::new()),
        connection_status: Mutex::new(HashMap::new()),
        job_completion_timeout,
    });

    let cloned_connection = connections.clone();
    let cloned_sender = sender.clone();
    let wss_handle = tokio::spawn(async move {
        let total_connections = Arc::new(Mutex::new(ConnectionID(0u64)));
        loop {
            if let Ok((stream, _)) = listener.accept().await {
                let cloned_connection = cloned_connection.clone();
                let cloned_sender = cloned_sender.clone();
                let job_mtx = job_mutex.clone();
                let cloned_connection_id = total_connections.clone();
                tokio::spawn(async move {
                    cloned_connection
                        .accept_connection(cloned_connection_id, stream, cloned_sender, job_mtx)
                        .await
                });
            }
        }
    });

    let job_handle_job = tokio::spawn(async move {
        connections.handle_assigned_jobs(sender).await;
    });
    Ok((wss_handle, job_handle_job))
}
